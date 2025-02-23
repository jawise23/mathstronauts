use macroquad::prelude::*;

// Rename the external rand crate to avoid collisions with macroquad's built-in rand.
use ::rand as ext_rand;
use ext_rand::prelude::*;
use ext_rand::seq::SliceRandom;

#[derive(Debug)]
enum GameState {
    Menu,
    Playing,
    Pause(f32), // Pause duration (in seconds) after a correct answer.
    GameOver,
}

#[derive(PartialEq, Debug)]
enum PlayerState {
    Normal,
    Fail,
}

struct MultipleChoice {
    x: f32,
    y: f32,
    text: String,
    is_correct: bool,
}

struct Player {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    width: f32,
    height: f32,
    state: PlayerState,
}

struct Alien {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    speed: f32, // pixels per second
}

// Movement and layout constants.
const MOVE_SPEED: f32 = 3.0;
const BOOST: f32 = 0.3;
const GRAVITY: f32 = 0.2;
const GROUND_Y: f32 = 650.0; // New top edge of the ground area.
const GROUND_HEIGHT: f32 = 100.0; // New ground height.

// Alien wall: The alien is drawn at x=0 with width=40. We add a 10-pixel buffer.
const ALIEN_WALL_BUFFER: f32 = 10.0;
const ALIEN_WIDTH: f32 = 40.0;
const ALIEN_WALL: f32 = ALIEN_WIDTH + ALIEN_WALL_BUFFER; // 50

// Lives: starting number and life-box dimensions.
const INITIAL_LIVES: i32 = 10;
const LIFE_BOX_SIZE: f32 = 20.0;
const LIFE_BOX_SPACING: f32 = 5.0;

// 1) A helper function to draw centered text at a given Y position
fn draw_centered_text(text: &str, y: f32, font_size: u16, color: Color) {
    // Measure how wide the text is in pixels
    let dims = measure_text(text, None, font_size, 1.0);
    // Compute the X so that the text is centered horizontally
    let x = (screen_width() - dims.width) / 2.0;
    // Draw the text at (x, y)
    draw_text(text, x, y, font_size as f32, color);
}

// 2) A function to draw the menu screen
fn draw_menu() {
    // Clear background
    clear_background(SKYBLUE);

    // Draw each line of menu text, centered horizontally
    draw_centered_text("Math Game", screen_height() / 2.0 - 150.0, 60, BLACK);
    draw_centered_text(
        "Select Difficulty Level:",
        screen_height() / 2.0 - 50.0,
        40,
        BLACK,
    );
    draw_centered_text(
        "0: Easy    1: Medium    2: Hard    3: Very Hard",
        screen_height() / 2.0,
        40,
        BLACK,
    );
    draw_centered_text(
        "Press the corresponding number key to start",
        screen_height() / 2.0 + 50.0,
        30,
        DARKGRAY,
    );
}

// Configure the game settings.
fn conf() -> Conf {
    Conf {
        window_title: "Math Game".to_owned(),
        window_width: 1024, // Set your desired window width here.
        window_height: 768, // Set your desired window height here.
        ..Default::default()
    }
}
// Create a fresh player starting at x = ALIEN_WALL, on the ground.
fn new_player() -> Player {
    Player {
        x: ALIEN_WALL,
        y: GROUND_Y,
        vx: 0.0,
        vy: 0.0,
        width: 30.0,
        height: 30.0,
        state: PlayerState::Normal,
    }
}

/// Generates a new math question and four multiple-choice answers.
/// The difficulty increases with score: operands are chosen from 1 to max_number,
/// where max_number = 10 + (score/500)*10. (For example, if score is 0, max_number=10;
/// if score is 500, max_number=20; if score is 1000, max_number=30, etc.)
/// All numbers are generated from 1 (0 is excluded).
fn generate_question(score: i32) -> (String, Vec<MultipleChoice>) {
    let mut rng = ext_rand::thread_rng();
    let max_number = 10 + (score / 500) * 10;
    let num1 = rng.gen_range(1..=max_number);
    let num2 = rng.gen_range(1..=max_number);
    let correct_answer = num1 + num2;
    let question_str = format!("{} + {} = ?", num1, num2);

    let mut answers: Vec<MultipleChoice> = Vec::new();
    // Correct answer.
    answers.push(MultipleChoice {
        x: 0.0,
        y: 0.0,
        text: correct_answer.to_string(),
        is_correct: true,
    });
    // Three wrong answers (generated from 1 to max_number*2, 0 excluded).
    for _ in 0..3 {
        let mut wrong = rng.gen_range(1..(max_number * 2));
        while wrong == correct_answer {
            wrong = rng.gen_range(1..(max_number * 2));
        }
        answers.push(MultipleChoice {
            x: 0.0,
            y: 0.0,
            text: wrong.to_string(),
            is_correct: false,
        });
    }
    // Shuffle answers.
    answers.shuffle(&mut rng);

    // Evenly space them across a horizontal margin.
    // For example, if you want a 100-pixel margin on each side:
    let margin = 100.0;
    let available_width = screen_width() - 2.0 * margin;
    let num_choices = answers.len() as f32;
    let slot_width = available_width / num_choices;
    // Each answer box has a fixed width of 100, so center each box in its slot.
    for (i, ans) in answers.iter_mut().enumerate() {
        ans.x = margin + slot_width * (i as f32 + 0.5) - 50.0; // 50 = 100/2
        ans.y = 200.0;
    }
    (question_str, answers)
}

/// Updates the alien's speed based on the current score.
/// Base speed is 50 pixels/second.
/// For score < 500, speed remains 50. For score ≥ 500, speed increases by 25 for every additional 1000 points.
fn update_alien_speed(alien: &mut Alien, score: i32) {
    let base_speed = 50.0;
    if score < 500 {
        alien.speed = base_speed;
    } else {
        let increments = 1.0 + ((score - 500) as f32 / 1000.0).floor();
        alien.speed = base_speed + increments * 25.0;
    }
}

#[macroquad::main(conf)]
async fn main() {
    // Start in Menu state.
    let mut game_state = GameState::Menu;
    // This variable will hold the starting score based on difficulty.
    let mut score = 0;
    // Lives.
    let mut lives = INITIAL_LIVES;
    // Variables for the question and choices.
    let mut question = String::new();
    let mut choices: Vec<MultipleChoice> = Vec::new();
    // Create the player.
    let mut player = new_player();
    // Create the alien.
    let mut alien = Alien {
        x: 0.0,
        y: 0.0,
        width: ALIEN_WIDTH,
        height: 40.0,
        speed: 50.0,
    };

    loop {
        match game_state {
            GameState::Menu => {
                draw_menu();

                // Check for key presses (0, 1, 2, 3).
                if is_key_pressed(KeyCode::Key0) {
                    score = 0; // Base difficulty.
                    game_state = GameState::Playing;
                    // Reset lives and player.
                    lives = INITIAL_LIVES;
                    player = new_player();
                    alien.y = 0.0;
                    let (q, c) = generate_question(score);
                    question = q;
                    choices = c;
                } else if is_key_pressed(KeyCode::Key1) {
                    score = 500; // As if achieved 500 points.
                    game_state = GameState::Playing;
                    lives = INITIAL_LIVES;
                    player = new_player();
                    alien.y = 0.0;
                    let (q, c) = generate_question(score);
                    question = q;
                    choices = c;
                } else if is_key_pressed(KeyCode::Key2) {
                    score = 1000; // As if achieved 1000 points.
                    game_state = GameState::Playing;
                    lives = INITIAL_LIVES;
                    player = new_player();
                    alien.y = 0.0;
                    let (q, c) = generate_question(score);
                    question = q;
                    choices = c;
                } else if is_key_pressed(KeyCode::Key3) {
                    score = 1500; // As if achieved 1500 points.
                    game_state = GameState::Playing;
                    lives = INITIAL_LIVES;
                    player = new_player();
                    alien.y = 0.0;
                    let (q, c) = generate_question(score);
                    question = q;
                    choices = c;
                }
            }
            GameState::Playing => {
                // Update player movement.
                update_player(&mut player);

                // Update alien speed and vertical position.
                update_alien_speed(&mut alien, score);
                alien.y += alien.speed * get_frame_time();
                if alien.y + alien.height >= GROUND_Y {
                    // Alien reached the ground: lose a life.
                    lives -= 1;
                    if lives <= 0 {
                        game_state = GameState::GameOver;
                    } else {
                        alien.y = 0.0;
                        player = new_player();
                        let (q, c) = generate_question(score);
                        question = q;
                        choices = c;
                    }
                }

                // Check collisions with answer boxes (only if player is in Normal state).
                if player.state == PlayerState::Normal {
                    let mut collided = false;
                    let mut correct_collision = false;
                    for choice in &choices {
                        if overlaps(
                            player.x,
                            player.y,
                            player.width,
                            player.height,
                            choice.x,
                            choice.y,
                            100.0,
                            80.0,
                        ) {
                            collided = true;
                            correct_collision = choice.is_correct;
                            break;
                        }
                    }
                    if collided {
                        if correct_collision {
                            score += 100;
                            game_state = GameState::Pause(0.5);
                        } else {
                            lives -= 1;
                            if lives <= 0 {
                                game_state = GameState::GameOver;
                            } else {
                                player.state = PlayerState::Fail;
                                println!("Wrong Answer!");
                            }
                        }
                    }
                }
                render_scene(&question, &choices, &player, score, &alien, lives);
            }
            GameState::Pause(ref mut time_left) => {
                *time_left -= get_frame_time();
                if *time_left <= 0.0 {
                    player = new_player();
                    alien.y = 0.0;
                    let (q, c) = generate_question(score);
                    question = q;
                    choices = c;
                    game_state = GameState::Playing;
                }
                render_scene(&question, &choices, &player, score, &alien, lives);
            }
            GameState::GameOver => {
                clear_background(SKYBLUE);
                let game_over_text = "GAME OVER";
                let score_str = format!("Score: {}", score);
                draw_text(
                    game_over_text,
                    screen_width() / 2.0 - 150.0,
                    screen_height() / 2.0,
                    60.0,
                    RED,
                );
                draw_text(
                    &score_str,
                    screen_width() / 2.0 - 100.0,
                    screen_height() / 2.0 + 80.0,
                    40.0,
                    BLACK,
                );
                draw_text(
                    "Press SPACE to return to Menu",
                    screen_width() / 2.0 - 200.0,
                    screen_height() / 2.0 + 140.0,
                    30.0,
                    DARKGRAY,
                );
                if is_key_pressed(KeyCode::Space) {
                    game_state = GameState::Menu;
                }
            }
        }
        next_frame().await;
    }
}

fn update_player(player: &mut Player) {
    match player.state {
        PlayerState::Normal => {
            if is_key_down(KeyCode::Left) {
                player.vx = -MOVE_SPEED;
            } else if is_key_down(KeyCode::Right) {
                player.vx = MOVE_SPEED;
            } else {
                player.vx = 0.0;
            }
            if is_key_down(KeyCode::Up) {
                player.vy -= BOOST;
            }
            player.vy += GRAVITY;
            player.x += player.vx;
            player.y += player.vy;
            let screen_w = screen_width();
            if player.x < ALIEN_WALL {
                player.x = ALIEN_WALL;
            }
            if player.x + player.width > screen_w {
                player.x = screen_w - player.width;
            }
            if player.y < 0.0 {
                player.y = 0.0;
                player.vy = 0.0;
            }
            if player.y + player.height > GROUND_Y + player.height {
                player.y = GROUND_Y;
                player.vy = 0.0;
            }
        }
        PlayerState::Fail => {
            player.vx = 0.0;
            player.vy += GRAVITY;
            player.x += player.vx;
            player.y += player.vy;
            if player.y + player.height > GROUND_Y + player.height {
                player.y = GROUND_Y;
                player.vy = 0.0;
                player.state = PlayerState::Normal;
            }
        }
    }
}

fn render_scene(
    question: &str,
    choices: &[MultipleChoice],
    player: &Player,
    score: i32,
    alien: &Alien,
    lives: i32,
) {
    clear_background(SKYBLUE);
    // Draw the ground.
    draw_rectangle(
        0.0,
        GROUND_Y + player.height,
        screen_width(),
        GROUND_HEIGHT,
        BROWN,
    );
    // Draw the question.
    draw_text(question, 50.0, 100.0, 50.0, BLACK);
    // Draw the score at top-right.
    let score_str = format!("Score: {}", score);
    let score_dimensions = measure_text(&score_str, None, 40, 1.0);
    let x_score = screen_width() - score_dimensions.width - 20.0;
    draw_text(&score_str, x_score, 50.0, 40.0, BLACK);
    // Draw the answer boxes.
    for choice in choices {
        draw_rectangle(choice.x, choice.y, 100.0, 80.0, GRAY);
        let text_x = choice.x + 15.0;
        let text_y = choice.y + 45.0;
        draw_text(&choice.text, text_x, text_y, 30.0, BLACK);
    }
    // Draw the player's rocket.
    draw_rectangle(player.x, player.y, player.width, player.height, RED);
    // Draw the alien.
    draw_rectangle(alien.x, alien.y, alien.width, alien.height, GREEN);
    // Draw lives as small red boxes inside the ground (bottom-left).
    let mut life_x = 10.0;
    let life_y = GROUND_Y + player.height + (GROUND_HEIGHT - LIFE_BOX_SIZE) / 2.0;
    for _ in 0..lives {
        draw_rectangle(life_x, life_y, LIFE_BOX_SIZE, LIFE_BOX_SIZE, RED);
        life_x += LIFE_BOX_SIZE + LIFE_BOX_SPACING;
    }
}

fn overlaps(ax: f32, ay: f32, aw: f32, ah: f32, bx: f32, by: f32, bw: f32, bh: f32) -> bool {
    ax < bx + bw && ax + aw > bx && ay < by + bh && ay + ah > by
}
