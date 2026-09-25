use console_core_color as color;

fn main() {
    let mut hue = 0.0;

    while hue < 360.0 {
        let mut lightness = 0.0;

        while lightness <= 1.0001 {
            let mut chroma = 0.0;

            while chroma <= 0.2001 {
                let asked = color::Oklch { lightness, chroma, hue };

                let Ok(code) = color::hexcode(asked);
                let Ok(back) = color::to_oklch(&code);
                let Ok(fitted) = color::fit(asked);
                let Ok(lifted) = color::lift(&code, 0.07);

                let (back_lightness, back_chroma, back_hue) = (back.lightness, back.chroma, back.hue);

                println!(
                    "hex {hue} {lightness} {chroma} {code} {fitted:.12} {back_lightness:.12} {back_chroma:.12} {back_hue:.12} {lifted}"
                );
                chroma += 0.025;
            }

            lightness += 0.05;
        }

        hue += 7.0;
    }

    let grounds = ["2a1a24".to_string(), "3d2833".to_string()];

    let said = |found: Result<f64, color::Short>| match found {
        Ok(value) => format!("{value:.15}"),
        Err(why) => why.0,
    };

    for hue in (0..360).step_by(11) {
        for ratio in [3.0, 4.5, 7.0, 10.0] {
            let floor = color::Floor { ratio, lightness_contrast: 0.0 };
            let from = color::Oklch { lightness: 0.0, chroma: 0.09, hue: f64::from(hue) };
            let Ok(top) = from.at(1.0);

            let light = color::lightest_clearing(from, &grounds, floor);
            let dark = color::darkest_clearing(top, &grounds, floor);
            println!(
                "clear {hue} {ratio} {} {}",
                said(light),
                said(dark)
            );
        }
    }

    let codes = ["000000", "ffffff", "7f7f7f", "808080", "ff8040", "123456", "fedcba"];

    for one in codes {
        for other in codes {
            let Ok(contrast) = color::contrast(color::HexColor(one), color::Ground(other));

            println!("contrast {one} {other} {contrast:.12}");

            for alpha in [0.0, 0.075, 0.26, 0.5, 0.52, 0.88, 1.0] {
                let Ok(over) = color::over(color::HexColor(one), color::Ground(other), alpha);

                println!("over {one} {other} {alpha} {over}");
            }
        }

        let Ok(luminance) = color::luminance(one);

        println!("lum {one} {luminance:.15}");
    }
}
