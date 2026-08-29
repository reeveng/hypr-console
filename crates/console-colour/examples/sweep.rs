// Prints what every function in the crate answers over a wide sweep, so the
// same sweep taken from somewhere else can be diffed against it.
use console_colour as col;

fn main() {
    let mut hue = 0.0;
    while hue < 360.0 {
        let mut lightness = 0.0;
        while lightness <= 1.0001 {
            let mut chroma = 0.0;
            while chroma <= 0.2001 {
                let code = col::hexcode(lightness, chroma, hue);
                let (l, c, h) = col::to_oklch(&code);
                println!(
                    "hex {hue} {lightness} {chroma} {code} {:.12} {:.12} {:.12} {:.12} {}",
                    col::fit(lightness, chroma, hue),
                    l, c, h,
                    col::lift(&code, 0.07)
                );
                chroma += 0.025;
            }
            lightness += 0.05;
        }
        hue += 7.0;
    }
    let grounds = ["2a1a24".to_string(), "3d2833".to_string()];
    for hue in (0..360).step_by(11) {
        for ratio in [3.0, 4.5, 7.0, 10.0] {
            let light = col::lightest_clearing(0.09, hue as f64, &grounds, ratio, 0.0);
            let dark = col::darkest_clearing(0.09, hue as f64, &grounds, ratio);
            println!(
                "clear {hue} {ratio} {} {}",
                light.map(|v| format!("{v:.15}")).unwrap_or_else(|e| e.0),
                dark.map(|v| format!("{v:.15}")).unwrap_or_else(|e| e.0)
            );
        }
    }
    let codes = ["000000", "ffffff", "7f7f7f", "808080", "ff8040", "123456", "fedcba"];
    for one in codes {
        for other in codes {
            println!("contrast {one} {other} {:.12}", col::contrast(one, other));
            for alpha in [0.0, 0.075, 0.26, 0.5, 0.52, 0.88, 1.0] {
                println!("over {one} {other} {alpha} {}", col::over(one, other, alpha));
            }
        }
        println!("lum {one} {:.15}", col::luminance(one));
    }
}
