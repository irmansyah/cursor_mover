use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::TextureCreator;
use sdl2::sys::SDL_SetWindowOpacity;
use sdl2::video::Window;
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixListener;
use std::process::{self};
use std::sync::{Arc, Mutex};
use std::thread;

const SOCKET_PATH: &str = "/tmp/cursor_mover_socket";

// Predefined 13x13 grid with two-character combinations
const GRID_LABELS: [[&str; 13]; 13] = [
    [
        "aa", "ab", "ac", "ad", "ae", "af", "ag", "ah", "ai", "aj", "ak", "al", "am",
    ],
    [
        "ba", "bb", "bc", "bd", "be", "bf", "bg", "bh", "bi", "bj", "bk", "bl", "bm",
    ],
    [
        "ca", "cb", "cc", "cd", "ce", "cf", "cg", "ch", "ci", "cj", "ck", "cl", "cm",
    ],
    [
        "da", "db", "dc", "dd", "de", "df", "dg", "dh", "di", "dj", "dk", "dl", "dm",
    ],
    [
        "ea", "eb", "ec", "ed", "ee", "ef", "eg", "eh", "ei", "ej", "ek", "el", "em",
    ],
    [
        "fa", "fb", "fc", "fd", "fe", "ff", "fg", "fh", "fi", "fj", "fk", "fl", "fm",
    ],
    [
        "ga", "gb", "gc", "gd", "ge", "gf", "gg", "gh", "gi", "gj", "gk", "gl", "gm",
    ],
    [
        "ha", "hb", "hc", "hd", "he", "hf", "hg", "hh", "hi", "hj", "hk", "hl", "hm",
    ],
    [
        "ia", "ib", "ic", "id", "ie", "if", "ig", "ih", "ii", "ij", "ik", "il", "im",
    ],
    [
        "ja", "jb", "jc", "jd", "je", "jf", "jg", "jh", "ji", "jj", "jk", "jl", "jm",
    ],
    [
        "ka", "kb", "kc", "kd", "ke", "kf", "kg", "kh", "ki", "kj", "kk", "kl", "km",
    ],
    [
        "la", "lb", "lc", "ld", "le", "lf", "lg", "lh", "li", "lj", "lk", "ll", "lm",
    ],
    [
        "ma", "mb", "mc", "md", "me", "mf", "mg", "mh", "mi", "mj", "mk", "ml", "mm",
    ],
];

// let subgrid_width_size = 7;
// let subgrid_height_size = 4;
const SUB_GRID_LABELS: [[&str; 6]; 4] = [
    ["ed", "ee", "ef", "eg", "eh", "ei"],
    ["fd", "fe", "ff", "fg", "fh", "fi"],
    ["gd", "ge", "gf", "gg", "gh", "gi"],
    ["hd", "he", "hf", "hg", "hh", "hi"],
];

fn set_window_transparency(window: &Window, opacity: f32) {
    // Convert the opacity to a valid range (0.0 to 1.0) - e.g., 50% opacity = 0.5
    unsafe {
        SDL_SetWindowOpacity(window.raw(), opacity);
    }
}

fn main() {
    // Start the background listener
    println!("Starting background listener...");
    start_listener();
}

/// Starts the background listener for commands
fn start_listener() {
    // Clean up any existing socket
    let _ = std::fs::remove_file(SOCKET_PATH);

    // Create a Unix socket to listen for commands
    let listener = UnixListener::bind(SOCKET_PATH).unwrap_or_else(|e| {
        eprintln!("Failed to create socket: {}", e);
        process::exit(1);
    });

    println!("Listening for commands on {}", SOCKET_PATH);

    let window_shown = Arc::new(Mutex::new(false));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let mut reader = BufReader::new(stream);

                let mut command = String::new();
                if reader.read_line(&mut command).is_ok() {
                    let command = command.trim();
                    match command {
                        "cursor_mover_show" => {
                            let mut shown = window_shown.lock().unwrap();
                            if !*shown {
                                println!("Received 'show' command. Displaying overlay...");
                                *shown = true;

                                let window_clone = Arc::clone(&window_shown);
                                thread::spawn(move || show_cursor_mover(window_clone));
                            }
                        }
                        "cursor_mover_hide" => {
                            let mut shown = window_shown.lock().unwrap();
                            if *shown {
                                println!("Received 'hide' command. Displaying overlay...");
                                *shown = false;
                            }
                        }
                        _ => {
                            println!("Unknown command: {}", command);
                        }
                    }
                }
            }
            Err(e) => eprintln!("Connection error: {}", e),
        }
    }
}

fn show_cursor_mover(window_shown: Arc<Mutex<bool>>) {
    let sdl_context = sdl2::init().unwrap();
    let ttf_context = sdl2::ttf::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let screen_width = 2560;
    let screen_height = 1440;

    let cols = 13;
    let rows = 13;
    let cell_width = screen_width / cols;
    let cell_height = screen_height / rows;

    let subgrid_width_size = 6;
    let subgrid_height_size = 4;
    let subgrid_cell_width = cell_width / subgrid_width_size;
    let subgrid_cell_height = cell_height / subgrid_height_size;

    let window = video_subsystem
        .window("Overlay", screen_width, screen_height)
        .position_centered()
        .borderless()
        .build()
        .unwrap();

    let mut canvas = window.clone().into_canvas().build().unwrap();
    let texture_creator: TextureCreator<_> = canvas.texture_creator();

    set_window_transparency(&window.clone(), 0.2);

    // Load font
    let font_path = "/usr/share/fonts/TTF/FiraCode-Medium.ttf";
    let font = ttf_context.load_font(font_path, 80).unwrap();
    let font_sub = ttf_context.load_font(font_path, 16).unwrap();

    let mut event_pump = sdl_context.event_pump().unwrap();

    // State to track subgrid display
    let mut user_input = String::new();
    let mut subgrid_display = None; // Stores (row, col) of the selected cell for subgrid

    while *window_shown.lock().unwrap() {
        // Check for events like pressing the Escape key or typing
        for event in event_pump.poll_iter() {
            match event {
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    println!("Escape key pressed. Hiding overlay...");
                    let mut shown = window_shown.lock().unwrap();
                    *shown = false;
                    return; // Exit the rendering loop
                }
                Event::TextInput { text, .. } => {
                    user_input.push_str(&text);
                    if user_input.len() == 2 {
                        // Check for valid input
                        let mut found = false;
                        for (row, row_labels) in GRID_LABELS.iter().enumerate() {
                            if let Some(col) =
                                row_labels.iter().position(|&label| label == user_input)
                            {
                                println!(
                                    "Selected cell: {} (row {}, col {})",
                                    user_input, row, col
                                );
                                subgrid_display = Some((row, col)); // Show subgrid
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            println!("Invalid input: {}", user_input);
                        }
                        user_input.clear();
                    }
                }
                _ => {}
            }
        }

        // Draw the main grid
        canvas.set_draw_color(Color::RGBA(0, 0, 0, 64));
        canvas.clear();

        for col in 0..cols {
            for row in 0..rows {
                let x = col * cell_width;
                let y = row * cell_height;

                // Draw grid cell
                canvas.set_draw_color(Color::RGBA(255, 255, 255, 192));
                canvas
                    .draw_rect(Rect::new(x as i32, y as i32, cell_width, cell_height))
                    .unwrap();

                // Get the key combination for this grid cell
                let key = GRID_LABELS[row as usize][col as usize];

                // Render the text
                let surface = font
                    .render(key)
                    .blended(sdl2::pixels::Color::RGBA(255, 255, 255, 192))
                    .unwrap();
                let texture = texture_creator
                    .create_texture_from_surface(&surface)
                    .unwrap();

                let text_width = surface.width();
                let text_height = surface.height();

                // Center the text in the grid cell
                let text_x = (x as i32 + (cell_width as i32) / 2 - (text_width / 2) as i32) as i32;
                let text_y =
                    (y as i32 + (cell_height as i32) / 2 - (text_height / 2) as i32) as i32;

                let target_rect = Rect::new(text_x, text_y, text_width, text_height);

                canvas.copy(&texture, None, target_rect).unwrap();
            }
        }


        // If a subgrid is selected, draw it inside the selected cell
        if let Some((row, col)) = subgrid_display {
            let base_x = col * cell_width as usize;
            let base_y = row * cell_height as usize;

            for sub_col in 0..subgrid_width_size {
                for sub_row in 0..subgrid_height_size {
                    let x = base_x + sub_col as usize * subgrid_cell_width as usize;
                    let y = base_y + sub_row as usize * subgrid_cell_height as usize;

                    // Draw subgrid cell
                    //
                    canvas.set_draw_color(Color::RGBA(255, 255, 255, 192));
                    canvas
                        .draw_rect(Rect::new(
                            x as i32,
                            y as i32,
                            subgrid_cell_width,
                            subgrid_cell_height,
                        ))
                        .unwrap();

                    let key_sub = SUB_GRID_LABELS[sub_row as usize][sub_col as usize];
                    let surface = font_sub
                        .render(&key_sub)
                        .blended(sdl2::pixels::Color::RGBA(255, 255, 255, 192))
                        .unwrap();
                    let texture = texture_creator
                        .create_texture_from_surface(&surface)
                        .unwrap();

                    let text_width = surface.width();
                    let text_height = surface.height();

                    let text_x =
                        x as i32 + (subgrid_cell_width as i32) / 2 - (text_width / 2) as i32;
                    let text_y =
                        y as i32 + (subgrid_cell_height as i32) / 2 - (text_height / 2) as i32;

                    let target_rect = Rect::new(text_x, text_y, text_width, text_height);

                    canvas.copy(&texture, None, target_rect).unwrap();
                }
            }
        }

        canvas.present();

        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    println!("Exiting overlay rendering loop.");
}

/// Moves the cursor to the specified screen position.
fn move_cursor(window: &Window, x: i32, y: i32) {
    let sdl_context = sdl2::init().unwrap();
    let mouse_util = sdl_context.mouse();
    mouse_util.warp_mouse_in_window(window, x, y);
}
// fn show_cursor_mover(window_shown: Arc<Mutex<bool>>) {
//     let sdl_context = sdl2::init().unwrap();
//     let ttf_context = sdl2::ttf::init().unwrap();
//     let video_subsystem = sdl_context.video().unwrap();

//     let screen_width = 2560;
//     let screen_height = 1440;

//     let cols = 13;
//     let rows = 13;
//     let cell_width = screen_width / cols;
//     let cell_height = screen_height / rows;

//     let window = video_subsystem
//         .window("Overlay", screen_width, screen_height)
//         .position_centered()
//         .borderless()
//         .build()
//         .unwrap();

//     let mut canvas = window.clone().into_canvas().build().unwrap();
//     let texture_creator: TextureCreator<_> = canvas.texture_creator();

//     set_window_transparency(&window.clone(), 0.2);

//     // Load font
//     let font_path = "/usr/share/fonts/TTF/FiraCode-Bold.ttf";
//     let font = ttf_context.load_font(font_path, 86).unwrap();

//     let mut event_pump = sdl_context.event_pump().unwrap();
//     let mut user_input = String::new();

//     while *window_shown.lock().unwrap() {
//         // Check for events
//         for event in event_pump.poll_iter() {
//             match event {
//                 Event::KeyDown {
//                     keycode: Some(Keycode::Escape),
//                     ..
//                 } => {
//                     println!("Escape key pressed. Hiding overlay...");
//                     let mut shown = window_shown.lock().unwrap();
//                     *shown = false;
//                     return; // Exit the rendering loop
//                 }
//                 Event::TextInput { text, .. } => {
//                     user_input.push_str(&text);
//                     if user_input.len() == 2 {
//                         // Check if the input matches a grid label
//                         let mut found = false;
//                         for (row, row_labels) in GRID_LABELS.iter().enumerate() {
//                             if let Some(col) =
//                                 row_labels.iter().position(|&label| label == user_input)
//                             {
//                                 // Move cursor to the corresponding grid cell
//                                 let cursor_x =
//                                     (col * cell_width as usize + cell_width as usize / 2) as i32;
//                                 let cursor_y =
//                                     (row * cell_height as usize + cell_height as usize / 2) as i32;

//                                 println!("Moving cursor to: {}, {}", cursor_x, cursor_y);
//                                 move_cursor(&window, cursor_x, cursor_y);
//                                 found = true;
//                                 break;
//                             }
//                         }
//                         if !found {
//                             println!("Invalid input: {}", user_input);
//                         }
//                         user_input.clear();

//                         let mut shown = window_shown.lock().unwrap();
//                         if *shown {
//                             println!("Received 'hide' command. Displaying overlay...");
//                             *shown = false;
//                         }
//                     }
//                 }
//                 _ => {}
//             }
//         }

//         // Draw the grid
//         canvas.set_draw_color(Color::RGBA(0, 0, 0, 128)); // Semi-transparent black
//         canvas.clear();

//         for col in 0..cols {
//             for row in 0..rows {
//                 let x = col * cell_width;
//                 let y = row * cell_height;

//                 // Draw grid cell
//                 canvas.set_draw_color(Color::RGBA(255, 255, 255, 255));
//                 canvas
//                     .draw_rect(Rect::new(x as i32, y as i32, cell_width, cell_height))
//                     .unwrap();

//                 // Get the key combination for this grid cell
//                 let key = GRID_LABELS[row as usize][col as usize];

//                 // Render the text
//                 let surface = font
//                     .render(key)
//                     .blended(sdl2::pixels::Color::RGBA(255, 255, 255, 255))
//                     .unwrap();
//                 let texture = texture_creator
//                     .create_texture_from_surface(&surface)
//                     .unwrap();

//                 let text_width = surface.width();
//                 let text_height = surface.height();

//                 // Center the text in the grid cell
//                 let text_x = (x as i32 + (cell_width as i32) / 2 - (text_width / 2) as i32) as i32;
//                 let text_y =
//                     (y as i32 + (cell_height as i32) / 2 - (text_height / 2) as i32) as i32;

//                 let target_rect = Rect::new(text_x, text_y, text_width, text_height);

//                 canvas.copy(&texture, None, target_rect).unwrap();
//             }
//         }

//         canvas.present();

//         std::thread::sleep(std::time::Duration::from_millis(100));
//     }

//     println!("Exiting overlay rendering loop.");
// }

