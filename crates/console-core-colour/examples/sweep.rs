use console_core_colour as col;

fn main() {
    let mut hue = 0.0;

    while hue < 360.0 {
        let mut lightness = 0.0;

        while lightness <= 1.0001 {
            let mut chroma = 0.0;

            while chroma <= 0.2001 {
                let asked = col::Oklch { lightness, chroma, hue };

                let Ok(code) = col::hexcode(asked);
                let Ok(back) = col::to_oklch(&code);
                let Ok(fitted) = col::fit(asked);
                let Ok(lifted) = col::lift(&code, 0.07);

                let (l, c, h) = (back.lightness, back.chroma, back.hue);

                println!(
                    "hex {hue} {lightness} {chroma} {code} {fitted:.12} {l:.12} {c:.12} {h:.12} {lifted}"
                );
                chroma += 0.025;
            }

            lightness += 0.05;
        }

        hue += 7.0;
    }

    let grounds = ["2a1a24".to_string(), "3d2833".to_string()];

    let said = |found: Result<f64, col::Short>| match found {
        Ok(value) => format!("{value:.15}"),
        Err(why) => why.0,
    };

    for hue in (0..360).step_by(11) {
        for ratio in [3.0, 4.5, 7.0, 10.0] {
            let floor = col::Floor { ratio, lc: 0.0 };
            let from = col::Oklch { lightness: 0.0, chroma: 0.09, hue: f64::from(hue) };
            let Ok(top) = from.at(1.0);

            let light = col::lightest_clearing(from, &grounds, floor);
            let dark = col::darkest_clearing(top, &grounds, floor);
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
            let Ok(contrast) = col::contrast(col::Ink(one), col::Ground(other));

            println!("contrast {one} {other} {contrast:.12}");

            for alpha in [0.0, 0.075, 0.26, 0.5, 0.52, 0.88, 1.0] {
                let Ok(over) = col::over(col::Ink(one), col::Ground(other), alpha);

                println!("over {one} {other} {alpha} {over}");
            }
        }

        let Ok(luminance) = col::luminance(one);

        println!("lum {one} {luminance:.15}");
    }
}
