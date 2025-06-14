#[macro_use] extern crate rocket;

use rocket::fs::NamedFile;
use rocket::response::status::NotFound;
use rocket::State;
use rocket::http::ContentType;
use std::path::{Path, PathBuf};
use std::io::Cursor;
use image::{DynamicImage, ImageFormat, ImageError};
use image::imageops::FilterType;

#[derive(Debug)]
struct Config {
    image_dir: PathBuf,
}

#[derive(FromForm, Clone)]
struct ImageParams {
    w: Option<u32>,
    h: Option<u32>,
    q: Option<u8>, // quality/compression (0-100)
}

impl ImageParams {
    // Generate cache directory name based on parameters
    fn cache_dir_name(&self) -> Option<String> {
        match (self.w, self.h, self.q) {
            (Some(w), Some(h), Some(q)) => Some(format!("cache/w{}h{}q{}", w, h, q)),
            (Some(w), Some(h), None) => Some(format!("cache/w{}h{}", w, h)),
            (Some(w), None, Some(q)) => Some(format!("cache/w{}q{}", w, q)),
            (None, Some(h), Some(q)) => Some(format!("cache/h{}q{}", h, q)),
            (None, Some(h), None) => Some(format!("cache/h{}", h)),
            (Some(w), None, None) => Some(format!("cache/w{}", w)),
            (None, None, Some(q)) => Some(format!("cache/q{}", q)),
            (None, None, None) => None,  // No parameters = no cache directory
        }
    }
}

#[get("/")]
async fn index() -> Result<NamedFile, NotFound<String>> {
    // Get the path relative to the executable
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    
    // Try multiple possible locations for index.html
    let possible_paths = vec![
        PathBuf::from("index.html"),  // CWD
        exe_dir.join("index.html"),    // Next to executable
        exe_dir.join("../../index.html"), // Project root (when running from target/debug)
    ];
    
    for path in possible_paths {
        if let Ok(file) = NamedFile::open(&path).await {
            return Ok(file);
        }
    }
    
    Err(NotFound("index.html not found".to_string()))
}

#[get("/<path..>?<params..>")]
async fn serve_image(
    path: PathBuf,
    params: Option<ImageParams>,
    config: &State<Config>,
) -> Result<(ContentType, Vec<u8>), NotFound<String>> {
    // Security check for path - no parent directory traversal
    for component in path.components() {
        if let std::path::Component::ParentDir = component {
            return Err(NotFound("Invalid path".to_string()));
        }
    }

    // Determine content type from extension
    let content_type = match path.extension().and_then(|s| s.to_str()) {
        Some("png") => ContentType::PNG,
        Some("jpg") | Some("jpeg") => ContentType::JPEG,
        Some("gif") => ContentType::GIF,
        Some("webp") => ContentType::WEBP,
        Some("bmp") => ContentType::BMP,
        _ => ContentType::PNG,
    };

    // Check if we need to process the image
    if let Some(ref params) = params {
        if let Some(cache_dir) = params.cache_dir_name() {
            // Create cache path maintaining directory structure
            let cache_base = config.image_dir.join(&cache_dir);
            let cache_path = cache_base.join(&path);
            
            if cache_path.exists() && cache_path.is_file() {
                // Serve from cache
                println!("Serving from cache: {:?}", cache_path);
                if let Ok(data) = std::fs::read(&cache_path) {
                    return Ok((content_type, data));
                }
            }
            
            // Need to process and cache
            let original_path = config.image_dir.join(&path);
            
            if !original_path.exists() || !original_path.is_file() {
                return Err(NotFound(format!("Image not found: {:?}", path)));
            }

            // Load and process the image
            let img = match image::open(&original_path) {
                Ok(img) => img,
                Err(e) => return Err(NotFound(format!("Failed to load image: {}", e))),
            };

            let quality = params.q;
            let processed_img = process_image(img, params.clone());

            // Convert to bytes
            let image_data = encode_image(&processed_img, &path, quality)?;

            // Create cache directory structure (including subdirectories)
            if let Some(parent) = cache_path.parent() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    eprintln!("Failed to create cache directory: {}", e);
                } else if let Err(e) = std::fs::write(&cache_path, &image_data) {
                    eprintln!("Failed to write to cache: {}", e);
                } else {
                    println!("Cached processed image: {:?}", cache_path);
                }
            }

            return Ok((content_type, image_data));
        }
    }

    // No parameters, serve original
    let original_path = config.image_dir.join(&path);
    
    if !original_path.exists() || !original_path.is_file() {
        return Err(NotFound(format!("Image not found: {:?}", path)));
    }

    match std::fs::read(&original_path) {
        Ok(data) => Ok((content_type, data)),
        Err(_) => Err(NotFound(format!("Failed to read image: {:?}", path))),
    }
}

fn encode_image(img: &DynamicImage, filename: &Path, quality: Option<u8>) -> Result<Vec<u8>, NotFound<String>> {
    // Determine the output format based on file extension
    let format = match filename.extension().and_then(|s| s.to_str()) {
        Some("png") => ImageFormat::Png,
        Some("jpg") | Some("jpeg") => ImageFormat::Jpeg,
        Some("gif") => ImageFormat::Gif,
        Some("webp") => ImageFormat::WebP,
        Some("bmp") => ImageFormat::Bmp,
        _ => ImageFormat::Png,
    };

    let mut buffer = Cursor::new(Vec::new());
    
    // Apply compression if it's a JPEG and quality is specified
    if matches!(format, ImageFormat::Jpeg) && quality.is_some() {
        let jpeg_quality = quality.unwrap().min(100);
        
        // For JPEG with quality, we need to encode it specially
        let rgba_image = img.to_rgba8();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buffer, jpeg_quality);
        
        if let Err(e) = encoder.encode(
            &rgba_image,
            rgba_image.width(),
            rgba_image.height(),
            image::ColorType::Rgba8,
        ) {
            return Err(NotFound(format!("Failed to encode image: {}", e)));
        }
    } else {
        // For other formats or JPEG without quality parameter
        if let Err(e) = img.write_to(&mut buffer, format) {
            return Err(NotFound(format!("Failed to encode image: {}", e)));
        }
    }

    Ok(buffer.into_inner())
}

fn process_image(mut img: DynamicImage, params: ImageParams) -> DynamicImage {
    // Handle resizing
    match (params.w, params.h) {
        (Some(w), Some(h)) => {
            // Both width and height specified - resize to exact dimensions
            img = img.resize_exact(w, h, FilterType::Lanczos3);
        }
        (Some(w), None) => {
            // Only width specified - maintain aspect ratio
            let ratio = w as f32 / img.width() as f32;
            let h = (img.height() as f32 * ratio) as u32;
            img = img.resize_exact(w, h, FilterType::Lanczos3);
        }
        (None, Some(h)) => {
            // Only height specified - maintain aspect ratio
            let ratio = h as f32 / img.height() as f32;
            let w = (img.width() as f32 * ratio) as u32;
            img = img.resize_exact(w, h, FilterType::Lanczos3);
        }
        (None, None) => {
            // No resizing needed
        }
    }

    img
}

#[get("/<path..>", rank = 2)]
async fn serve_original_image(
    path: PathBuf,
    config: &State<Config>,
) -> Result<NamedFile, NotFound<String>> {
    let image_path = config.image_dir.join(&path);
    
    // Security check - ensure the resolved path is within the image directory
    let canonical_base = config.image_dir.canonicalize()
        .map_err(|_| NotFound("Invalid base directory".to_string()))?;
    let canonical_image = image_path.canonicalize()
        .map_err(|_| NotFound("Image not found".to_string()))?;
    
    if !canonical_image.starts_with(&canonical_base) {
        return Err(NotFound("Invalid path".to_string()));
    }

    NamedFile::open(&image_path)
        .await
        .map_err(|_| NotFound(format!("Image not found: {:?}", path)))
}

#[launch]
fn rocket() -> _ {
    // Get image directory - also make it more flexible
    let image_dir = if let Ok(dir) = std::env::var("IMAGE_DIR") {
        PathBuf::from(dir)
    } else {
        // Try to find images directory relative to executable
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));
        
        // Check common locations
        let possible_dirs = vec![
            PathBuf::from("./images"),
            exe_dir.join("images"),
            exe_dir.join("../../images"),
        ];
        
        possible_dirs.into_iter()
            .find(|p| p.exists())
            .unwrap_or_else(|| PathBuf::from("./images"))
    };
    
    // Create the directory if it doesn't exist
    if !image_dir.exists() {
        std::fs::create_dir_all(&image_dir)
            .expect("Failed to create image directory");
    }

    let config = Config { image_dir: image_dir.clone() };

    println!("Serving images from: {:?}", image_dir);
    println!("Current working directory: {:?}", std::env::current_dir().ok());
    println!("Executable location: {:?}", std::env::current_exe().ok());

    rocket::build()
        .mount("/", routes![index, serve_image, serve_original_image])
        .manage(config)
}
