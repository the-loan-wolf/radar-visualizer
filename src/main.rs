use raylib::prelude::*;
use std::env;
use std::io::{self, BufRead, BufReader, Write};
use std::time::Duration;

// ===================== CONFIG =====================
const SCREEN_WIDTH: i32 = 1200;
const SCREEN_HEIGHT: i32 = 700;
const MAX_RANGE_CM: f32 = 40.0;

const BACKGROUND_COLOR: Color = Color::new(10, 15, 10, 255);
const FADE_ANIMATION_COLOR: Color = Color::new(0, 10, 0, 18);
const RADAR_OUTLINE: Color = Color::new(30, 120, 50, 255);
const DETECTED_OBJECT: Color = Color::new(255, 60, 60, 255);
const SWEEP_LINE_COLOR: Color = Color::new(150, 255, 170, 255);

const SWEEP_LINE_THICKNESS: f32 = 4.0;
const SWEEP_SPREAD_DEG: f32 = 3.0;
const SWEEP_STEP_DEG: f32 = 0.3;

// This happens at COMPILE time, putting the text inside your EXE
const SHADER_SOURCE: &str = include_str!("../shaders/radar_phosphor.fs");

fn main() {
    let args: Vec<String> = env::args().collect();

    let (port_name, baud_rate) = if args.len() >= 3 {
        // Option A: CLI Arguments
        let p = args[1].clone();
        let b = args[2].parse().unwrap_or(9600);
        println!("Using CLI arguments: Port: {}, Baud: {}", p, b);
        (p, b) // Return these to be assigned
    } else {
        // Option B: Interactive Fallback
        println!("\n--- Available Serial Ports ---");
        if let Ok(ports) = serialport::available_ports() {
            for p in ports {
                println!(" -> {}", p.port_name);
            }
        }

        print!("\nEnter Serial Port: ");
        io::stdout().flush().unwrap();
        let mut input_port = String::new();
        io::stdin()
            .read_line(&mut input_port)
            .expect("Failed to read line");

        print!("Enter Baud Rate (default 9600): ");
        io::stdout().flush().unwrap();
        let mut baud_str = String::new();
        io::stdin()
            .read_line(&mut baud_str)
            .expect("Failed to read line");

        (
            input_port.trim().to_string(),
            baud_str.trim().parse::<u32>().unwrap_or(9600),
        )
    };

    // ---- Initialize Raylib ----
    let (mut rl, thread) = raylib::init()
        .size(SCREEN_WIDTH, SCREEN_HEIGHT)
        .title("Radar - Arduino")
        .msaa_4x()
        .build();

    rl.set_target_fps(60);

    // ---- Open Serial ----
    let mut reader = serialport::new(&port_name, baud_rate)
        .timeout(Duration::from_millis(10))
        .open()
        .ok()
        .map(|port| BufReader::new(port));

    if reader.is_none() {
        println!("Warning: Failed to open serial port.");
    }

    // ---- Shader and Render Texture ----
    let mut shaders = rl.load_shader_from_memory(&thread, None, Some(SHADER_SOURCE));
    let intensity_loc = shaders.get_shader_location("intensity");
    shaders.set_shader_value(intensity_loc, 1.5f32);

    let mut target = rl
        .load_render_texture(&thread, SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32)
        .expect("Failed to create render texture");

    // Initial clear
    {
        let mut d = rl.begin_texture_mode(&thread, &mut target);
        d.clear_background(BACKGROUND_COLOR);
    }

    // ---- State ----
    let mut i_angle = 0.0f32;
    let mut i_distance = 0.0f32;
    let mut use_shader = true;
    let mut data_received = false;

    while !rl.window_should_close() {
        // ---- Input ----
        if rl.is_key_pressed(KeyboardKey::KEY_S) {
            use_shader = !use_shader;
        }

        if rl.is_window_resized() || rl.is_key_pressed(KeyboardKey::KEY_F) {
            // If 'F' was pressed, we toggle first, then wait a frame or
            // use the new dimensions immediately
            if rl.is_key_pressed(KeyboardKey::KEY_F) {
                rl.toggle_fullscreen();
            }

            let new_sw = rl.get_screen_width();
            let new_sh = rl.get_screen_height();

            // Re-create the texture at the FULL monitor resolution
            target = rl
                .load_render_texture(&thread, new_sw as u32, new_sh as u32)
                .expect("Failed to resize render texture");

            // Clear the new texture once so it doesn't start with garbage data
            let mut d = rl.begin_texture_mode(&thread, &mut target);
            d.clear_background(BACKGROUND_COLOR);
        }

        // ---- Calculate Responsive Geometry ----
        // Get current dimensions (works for both windowed and fullscreen)
        let current_sw = rl.get_screen_width() as f32;
        let current_sh = rl.get_screen_height() as f32;

        // Recalculate center and radius based on current screen size
        let radar_center = Vector2::new(current_sw / 2.0, current_sh * 0.926);
        let radar_radius = current_sw * 0.5;

        // ---- Read Serial ----
        if let Some(ref mut port) = reader {
            let mut line = String::new();
            if port.read_line(&mut line).is_ok() {
                let parts: Vec<&str> = line.trim().split(',').collect();
                if parts.len() == 2 {
                    if let (Ok(a), Ok(d)) = (parts[0].parse::<f32>(), parts[1].parse::<f32>()) {
                        i_angle = a;
                        i_distance = d;
                        data_received = true;
                    }
                }
            }
        }

        // ---- Draw to Texture (Persistence Layer) ----
        {
            let mut d = rl.begin_texture_mode(&thread, &mut target);

            // Persistence Fade Animation
            d.draw_rectangle(
                0,
                0,
                current_sw as i32,
                (current_sh as f32 * 0.926) as i32,
                FADE_ANIMATION_COLOR,
            );

            // Radar Arcs
            let arc_scales = [0.9375, 0.73, 0.521, 0.313];
            for scale in arc_scales {
                d.draw_circle_sector_lines(
                    radar_center,
                    (current_sw as f32 * scale) / 2.0,
                    180.0,
                    360.0,
                    128,
                    RADAR_OUTLINE,
                );
            }

            // Angle Markers
            for angle in (30..=150).step_by(30) {
                let rad = (angle as f32).to_radians();
                let line_end = Vector2::new(
                    radar_center.x - radar_radius * rad.cos(),
                    radar_center.y - radar_radius * rad.sin(),
                );
                d.draw_line_ex(radar_center, line_end, 2.0, RADAR_OUTLINE);

                let text_radius = radar_radius * 1.05;
                let text_pos = Vector2::new(
                    radar_center.x - text_radius * rad.cos(),
                    radar_center.y - text_radius * rad.sin(),
                );

                let display_angle = 180 - angle;
                let label = format!("{}", display_angle);
                let font_size = 20;
                let text_size = d.measure_text(&label, font_size);
                d.draw_text(
                    &label,
                    (text_pos.x - text_size as f32 / 2.0) as i32,
                    (text_pos.y - font_size as f32 / 2.0) as i32,
                    font_size,
                    RADAR_OUTLINE,
                );
            }

            if data_received {
                // Sweep Line
                let mut offset = -SWEEP_SPREAD_DEG;
                while offset <= 0.0 {
                    let a = (i_angle + offset).to_radians();
                    let sweep_end = Vector2::new(
                        radar_center.x + radar_radius * a.cos(),
                        radar_center.y - radar_radius * a.sin(),
                    );
                    d.draw_line_ex(
                        radar_center,
                        sweep_end,
                        SWEEP_LINE_THICKNESS,
                        SWEEP_LINE_COLOR,
                    );
                    offset += SWEEP_STEP_DEG;
                }

                // Detected Object
                let rad = i_angle.to_radians();
                if i_distance > 0.0 && i_distance < MAX_RANGE_CM {
                    let pixels_per_cm = radar_radius / MAX_RANGE_CM;
                    let pix_dist = i_distance * pixels_per_cm;

                    let object_pos = Vector2::new(
                        radar_center.x + pix_dist * rad.cos(),
                        radar_center.y - pix_dist * rad.sin(),
                    );
                    let edge_pos = Vector2::new(
                        radar_center.x + radar_radius * rad.cos(),
                        radar_center.y - radar_radius * rad.sin(),
                    );
                    d.draw_line_ex(object_pos, edge_pos, 6.0, DETECTED_OBJECT);
                }
            }
        }

        // ---- Final Render ----
        // This rectangle covers the ENTIRE window/screen
        let dest_rect = Rectangle::new(0.0, 0.0, current_sw, current_sh);

        // This rectangle selects the ENTIRE texture (inverted Y for Raylib textures)
        let source_rect = Rectangle::new(
            0.0,
            0.0,
            target.texture().width as f32,
            -target.texture().height as f32,
        );
        let mut d = rl.begin_drawing(&thread);
        d.clear_background(BACKGROUND_COLOR);

        if use_shader {
            let mut s_mode = d.begin_shader_mode(&mut shaders);
            s_mode.draw_texture_pro(
                target.texture(),
                source_rect,
                dest_rect,
                Vector2::zero(),
                0.0,
                Color::WHITE,
            );
        } else {
            d.draw_texture_pro(
                target.texture(),
                source_rect,
                dest_rect,
                Vector2::zero(),
                0.0,
                Color::WHITE,
            );
        }

        // UI Overlay
        // let ui_y_start = current_sh * 0.945;
        // d.draw_rectangle(
        //     0,
        //     (ui_y_start as f32) as i32,
        //     current_sw as i32,
        //     (current_sh as f32 * 0.070) as i32,
        //     Color::BLACK,
        // );
        d.draw_text(
            &format!("Angle: {:.0}", i_angle),
            (current_sw as f32 * 0.05) as i32,
            (current_sh as f32 * 0.95) as i32,
            30,
            RADAR_OUTLINE,
        );
        d.draw_text(
            &format!("Distance: {:.0} cm", i_distance),
            (current_sw as f32 * 0.75) as i32,
            (current_sh as f32 * 0.95) as i32,
            30,
            RADAR_OUTLINE,
        );
    }
}
