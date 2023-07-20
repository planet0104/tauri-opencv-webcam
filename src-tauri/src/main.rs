// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{num::NonZeroU32, sync::Mutex, path::PathBuf, fs};
use anyhow::{Result, anyhow};
use image::RgbImage;
use minifb::{Window, WindowOptions, Key, ScaleMode};
use nokhwa::{utils::{CameraIndex, RequestedFormat, RequestedFormatType}, pixel_format::RgbFormat, Camera};
use once_cell::sync::Lazy;
// use opencv::{prelude::*, imgcodecs::imwrite, core::{Vector, CV_8UC3}};
use tauri::Manager;
use time::{format_description, OffsetDateTime};
use windows::Win32::{UI::WindowsAndMessaging::{SetWindowPos, SWP_SHOWWINDOW}, Foundation::HWND};

// Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let _main_window = app.get_window("main").unwrap();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![greet, open_camera, close_camera, update_window_position, take_picture])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
async fn open_camera(camera_index: u32, width_percent: f32, height_percent: f32, offset_x_percent: f32, offset_y_percent: f32) -> std::result::Result<(), String> {
    println!("rust进入open_camera...");
    let mut o = CAMERA_OPENED.lock().map_err(|err| format!("相机锁定失败:{:?}", err))?;
    *o = true;
    println!("rust 启动相机线程...");
    std::thread::spawn(move ||{
        println!("启动相机...");
        let res = start_camera(camera_index, width_percent, height_percent, offset_x_percent, offset_y_percent);
        println!("相机关闭:{:?}", res);
    });
    Ok(())
}

#[tauri::command]
async fn update_window_position(x: isize, y: isize, width: usize, height: usize) -> std::result::Result<(), String>{
    set_camera_window_rect(x, y, width, height).map_err(|err| format!("窗口位置更新失败:{:?}", err))
}

#[tauri::command]
async fn close_camera() -> std::result::Result<(), String>{
    println!("rust进入close_camera...");
    let mut o = CAMERA_OPENED.lock().map_err(|err| format!("相机锁定失败:{:?}", err))?;
    *o = false;
    println!("rust结束close_camera...");
    Ok(())
}

#[tauri::command]
fn take_picture() -> std::result::Result<String, String>{
    println!("rust进入take_picture...");
    let mut c = CAMERA_INSTANCE.lock().map_err(|err| format!("相机锁定失败:{:?}", err))?;
    if c.is_none(){
        return Err(format!("相机未打开!"));
    }
    let camera = c.as_mut().unwrap();
    let frame = camera.frame().map_err(|err| format!("相机拍照失败:{:?}", err))?;
    let decoded_frame = frame.decode_image::<RgbFormat>().map_err(|err| format!("图像解码失败:{:?}", err))?;
    println!("拍照图片大小:{}x{}", decoded_frame.width(), decoded_frame.height());
    //文件名
    let local = OffsetDateTime::now_local().map_err(|err| format!("日期获取失败:{:?}", err))?;
    let format = format_description::parse("[year][month][day][hour][minute][second][subsecond]",).map_err(|err| format!("日期格式化失败:{:?}", err))?;
    let file_name = local.format(&format).map_err(|err| format!("日期格式化失败:{:?}", err))?;
    let file_name = format!("../{}.jpg", file_name);
    println!("拍照图片文件名:{file_name}");
    let img = RgbImage::from_raw(decoded_frame.width(), decoded_frame.height(), decoded_frame.to_vec()).unwrap();
    img.save(&file_name).map_err(|err| format!("图片保存失败:{:?}", err))?;
    let path_buf = PathBuf::from(file_name);
    Ok(fs::canonicalize(&path_buf).map_err(|err| format!("图片保存失败:{:?}", err))?.to_str().unwrap().to_string())
}

static CAMERA_INSTANCE: Lazy<Mutex<Option<Camera>>> = Lazy::new(|| { Mutex::new(None) });
static CAMERA_OPENED: Lazy<Mutex<bool>> = Lazy::new(|| { Mutex::new(false) });
static CAMERA_WINDOW_POSITION: Lazy<Mutex<(isize, isize, usize, usize)>> = Lazy::new(|| { Mutex::new((0, 0, 0, 0)) });

fn set_camera_window_rect(x: isize, y:isize, width: usize, height: usize) -> Result<()>{
    // println!("update_window_position:{x}x{y} {width}x{height}");
    match CAMERA_WINDOW_POSITION.lock(){
        Ok(mut pos) => {
            pos.0 = x;
            pos.1 = y;
            pos.2 = width;
            pos.3 = height;
            Ok(())
        }
        Err(err) => Err(anyhow!("{:?}", err))
    }
}

fn get_camera_window_rect() -> Result<(isize, isize, usize, usize)>{
    match CAMERA_WINDOW_POSITION.try_lock(){
        Ok(pos) => Ok(*pos),
        Err(err) => Err(anyhow!("{:?}", err))
    }
}

fn start_camera(camera_index: u32, width_percent: f32, height_percent: f32, offset_x_percent: f32, offset_y_percent: f32) -> Result<()>{
    let index = CameraIndex::Index(camera_index); 
    let requested = RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
    
    CAMERA_INSTANCE.lock().map_err(|err| anyhow!("{:?}", err))?.replace(Camera::new(index, requested)?);

    let (mut window_x, mut window_y, mut window_width, mut window_height) = get_camera_window_rect()?;

    let mut calc_pos = (
        (window_x as f32 + window_width as f32*offset_x_percent) as isize,
        (window_y as f32 + window_height as f32*offset_y_percent) as isize,
        (window_width as f32*width_percent) as usize,
        (window_height as f32*height_percent) as usize
    );

    println!("calc_pos={:?}", calc_pos);

    let mut buffer: Vec<u32> = vec![0; calc_pos.2 * calc_pos.3];

    let mut window = Window::new(
        "webcam",
        calc_pos.2,
        calc_pos.3,
        WindowOptions {
            resize: true,
            scale_mode: ScaleMode::Stretch,
            borderless: true,
            transparency: false,
            title: false,
            none: true,
            topmost:true,
            ..WindowOptions::default()
        },
    )?;

    window.set_position(calc_pos.0, calc_pos.1);

    window.limit_update_rate(Some(std::time::Duration::from_millis(1000/30)));

    // let mut decoded = None;

    while window.is_open() && !window.is_key_down(Key::Escape) {

        if let Ok((new_x, new_y, new_width, new_height)) = get_camera_window_rect(){
            if new_x != window_x || new_y != window_y || new_width != window_width || new_height != window_height{
                window_width = new_width;
                window_height = new_height;
                window_x = new_x;
                window_y = new_y;
                let new_calc_pos = (
                    (window_x as f32 + window_width as f32*offset_x_percent) as isize,
                    (window_y as f32 + window_height as f32*offset_y_percent) as isize,
                    (window_width as f32*width_percent) as usize,
                    (window_height as f32*height_percent) as usize
                );
                if new_calc_pos.2 != 0 && new_calc_pos.3 != 0{
                    calc_pos = new_calc_pos;
                    println!("刷新相机{:?}", calc_pos);
                    buffer = vec![0; calc_pos.2 * calc_pos.3];
                    // window.set_position(calc_pos.0, calc_pos.1);
                    let raw_handle = window.get_window_handle();
                    unsafe{
                        SetWindowPos(HWND(raw_handle as isize), None, calc_pos.0 as i32, calc_pos.1 as i32, calc_pos.2 as i32, calc_pos.3 as i32, SWP_SHOWWINDOW);
                    }
                }
            }
        }

        if let Ok(opened) = CAMERA_OPENED.try_lock(){
            if !*opened{
                println!("用户主动关闭相机, 结束窗口循环");
                break;
            }
        }

        if let Ok(mut camera) = CAMERA_INSTANCE.try_lock(){
            // let t = Instant::now();
            if camera.is_none(){
                println!("相机为空, 结束窗口循环");
                break;
            }
            let camera = camera.as_mut().unwrap();

            let frame = camera.frame()?;
            // decode into an ImageBuffer
            let mut decoded_frame = frame.decode_image::<RgbFormat>()?;

            // let t = Instant::now();

            let width = NonZeroU32::new(decoded_frame.width()).unwrap();
            let height = NonZeroU32::new(decoded_frame.height()).unwrap();
            let src_image = fast_image_resize::Image::from_slice_u8(
                width,
                height,
                &mut decoded_frame,
                fast_image_resize::PixelType::U8x3,
            )?;

            // Create container for data of destination image
            let dst_width = NonZeroU32::new(calc_pos.2 as u32).unwrap();
            let dst_height = NonZeroU32::new(calc_pos.3 as u32).unwrap();
            let mut dst_image = fast_image_resize::Image::new(
                dst_width,
                dst_height,
                src_image.pixel_type(),
            );

            // Get mutable view of destination image data
            let mut dst_view = dst_image.view_mut();

            // Create Resizer instance and resize source image
            // into buffer of destination image
            let mut resizer = fast_image_resize::Resizer::new(
                fast_image_resize::ResizeAlg::Convolution(fast_image_resize::FilterType::Bilinear),
            );
            resizer.resize(&src_image.view(), &mut dst_view)?;

            for (pixel, target) in dst_image.buffer().chunks(3).zip(buffer.iter_mut()){
                *target = u32::from_be_bytes([0, pixel[0], pixel[1], pixel[2]]);
            }

            // println!("耗时:{}ms", t.elapsed().as_millis());

            // decoded = Some(decoded_frame);
        }
        
        window.update_with_buffer(&buffer, calc_pos.2, calc_pos.3)?;
    }

    println!("相机窗口循环结束..");
    if let Ok(mut opened) = CAMERA_OPENED.lock(){
        *opened = false;
    }

    // if let Some(decoded) = decoded{
    //     let img = rgb_bytes_to_mat(&decoded, decoded.width(), decoded.height())?;
    //     imwrite("frame.png", &img, &Vector::<i32>::default())?;
    // }

    Ok(())
}

// fn rgb_bytes_to_mat(rgb_data: &[u8], width: u32, height: u32) -> Result<Mat> {
//     let mut rgb_data: Vec<u8> = rgb_data.to_vec();
//     // rgb 转 bgr
//     for pixel in rgb_data.chunks_mut(3) {
//         pixel.reverse();
//     }

//     // Create a cv::Mat with the same width, height and type as the RGB image
//     let mut mat = Mat::new_rows_cols_with_default(height as i32, width as i32, CV_8UC3, opencv::core::Scalar::all(0.0))?;

//     // Copy the RGB bytes to the cv::Mat
//     let rgb_slice = &rgb_data[..];
//     let mat_slice = mat.data_mut();
//     unsafe{ mat_slice.copy_from(rgb_slice.as_ptr(), rgb_slice.len()); }

//     // Return the cv::Mat
//     Ok(mat)
// }