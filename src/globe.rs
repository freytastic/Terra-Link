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

pub fn get_appearance(is_land: bool, intensity: f64) -> (char, Color) {
    if intensity > 0.0 {
        // Day side (Lambertian reflection)
        // Ambient light + diffuse light
        let i = 0.2 + (intensity * 0.8);

        let char = if is_land {
            '⣿' // land
        } else {
            // Waves on the water depending on intensity
            if i > 0.8 {
                '~'
            } else if i > 0.5 {
                '-'
            } else {
                '.'
            }
        };

        let color = if is_land {
            // green
            Color::Rgb(0, (255.0 * i) as u8, (50.0 * i) as u8)
        } else {
            // mlue
            Color::Rgb(0, (100.0 * i) as u8, (255.0 * i) as u8)
        };
        (char, color)
    } else {
        // Dim
        let n_i = (-intensity).clamp(0.0, 1.0);

        let char = if is_land { '⣿' } else { ' ' };
        let color = if is_land {
            // Fading out city lights or just faint land silhouette
            let dim = (40.0 * (1.0 - n_i)).max(15.0) as u8;
            Color::Rgb(0, dim, 0)
        } else {
            Color::Reset
        };
        (char, color)
    }
}
