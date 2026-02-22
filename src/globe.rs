use ratatui::style::Color;

pub const EARTH_MAP_WIDTH: usize = 64;
pub const EARTH_MAP_HEIGHT: usize = 32;

pub const EARTH_MAP: [&str; EARTH_MAP_HEIGHT] = [
    "                                                                ",
    "                                                                ",
    "        #############                                           ",
    "       #################                                        ",
    "     ######################          ##############             ",
    "    #########################      ##################           ",
    "   ###########################    #####################         ",
    "  #############################  ########################       ",
    "  #############################  ########################       ",
    "   ###########################    #######################       ",
    "    ####  ###################     #######################       ",
    "    ###    #################      ########################      ",
    "            ###############       ########################      ",
    "              ############        #######################       ",
    "              ###########         #######################       ",
    "               #########          #######################       ",
    "               ########            #####################        ",
    "               #######             ###################          ",
    "                ######             ###################          ",
    "                #####               ##################          ",
    "                 ####               #################           ",
    "                                     ###############            ",
    "                                     ##############             ",
    "                                      ############              ",
    "                                        ########                ",
    "                                         #####                  ",
    "                                          ###                   ",
    "                                           #             ####   ",
    "                                                        ######  ",
    "                                                         ####   ",
    "                                                                ",
    "                                                                ",
];

// Day side land: hot magenta/neon pink
const LAND_DAY_R: f64 = 255.0;
const LAND_DAY_G: f64 = 45.0;
const LAND_DAY_B: f64 = 149.0;

// Night side land: deep violet city glow
const LAND_NIGHT_BASE: f64 = 50.0;
const LAND_NIGHT_R_FACTOR: f64 = 0.6;
const LAND_NIGHT_B_FACTOR: f64 = 0.9;

// Day-side ocean: electric cyan
const OCEAN_DAY_R: f64 = 0.0;
const OCEAN_DAY_G: f64 = 200.0;
const OCEAN_DAY_B: f64 = 255.0;

// Night side ocean: dark abyss purple
const OCEAN_NIGHT_R: f64 = 10.0;
const OCEAN_NIGHT_G: f64 = 0.0;
const OCEAN_NIGHT_B: f64 = 25.0;

// Map a land/ocean point and its Lambertian intensity to a character and neon color.
pub fn get_appearance(is_land: bool, intensity: f64) -> (char, Color) {
    if intensity > 0.0 {
        // Day side : ambient + diffuse
        let i = 0.2 + (intensity * 0.8);

        let char = if is_land {
            '⣿'
        } else if i > 0.8 {
            '~'
        } else if i > 0.5 {
            '-'
        } else {
            '.'
        };

        let color = if is_land {
            Color::Rgb(
                (LAND_DAY_R * i) as u8,
                (LAND_DAY_G * i) as u8,
                (LAND_DAY_B * i) as u8,
            )
        } else {
            Color::Rgb(
                (OCEAN_DAY_R * i) as u8,
                (OCEAN_DAY_G * i) as u8,
                (OCEAN_DAY_B * i) as u8,
            )
        };
        (char, color)
    } else {
        // Night side
        let n_i = (-intensity).clamp(0.0, 1.0);

        let char = if is_land { '⣿' } else { ' ' };
        let color = if is_land {
            // Fading violet city light silhouette
            let dim = (LAND_NIGHT_BASE * (1.0 - n_i)).max(12.0);
            Color::Rgb(
                (dim * LAND_NIGHT_R_FACTOR) as u8,
                0,
                (dim * LAND_NIGHT_B_FACTOR) as u8,
            )
        } else {
            // Deep abyss purple tint
            let fade = 1.0 - n_i;
            Color::Rgb(
                (OCEAN_NIGHT_R * fade) as u8,
                (OCEAN_NIGHT_G * fade) as u8,
                (OCEAN_NIGHT_B * fade) as u8,
            )
        };
        (char, color)
    }
}
