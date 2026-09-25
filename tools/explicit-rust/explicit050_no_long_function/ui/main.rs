// UI test for EXPLICIT050 — a function fits on a screen.

// BAD EXPLICIT050 — a sequence longer than a screen, with no stages named.
//~v EXPLICIT050_NO_LONG_FUNCTION
fn long_walk(v0: u32) -> u32 {
    let v1 = v0 + 1;
    let v2 = v1 + 1;
    let v3 = v2 + 1;
    let v4 = v3 + 1;
    let v5 = v4 + 1;
    let v6 = v5 + 1;
    let v7 = v6 + 1;
    let v8 = v7 + 1;
    let v9 = v8 + 1;
    let v10 = v9 + 1;
    let v11 = v10 + 1;
    let v12 = v11 + 1;
    let v13 = v12 + 1;
    let v14 = v13 + 1;
    let v15 = v14 + 1;
    let v16 = v15 + 1;
    let v17 = v16 + 1;
    let v18 = v17 + 1;
    let v19 = v18 + 1;
    let v20 = v19 + 1;
    let v21 = v20 + 1;
    let v22 = v21 + 1;
    let v23 = v22 + 1;
    let v24 = v23 + 1;
    let v25 = v24 + 1;
    let v26 = v25 + 1;
    let v27 = v26 + 1;
    let v28 = v27 + 1;
    let v29 = v28 + 1;
    let v30 = v29 + 1;
    let v31 = v30 + 1;
    let v32 = v31 + 1;
    let v33 = v32 + 1;
    let v34 = v33 + 1;
    let v35 = v34 + 1;
    let v36 = v35 + 1;
    let v37 = v36 + 1;
    let v38 = v37 + 1;
    let v39 = v38 + 1;
    let v40 = v39 + 1;
    let v41 = v40 + 1;
    let v42 = v41 + 1;
    let v43 = v42 + 1;
    let v44 = v43 + 1;
    let v45 = v44 + 1;
    let v46 = v45 + 1;
    let v47 = v46 + 1;
    let v48 = v47 + 1;
    let v49 = v48 + 1;
    let v50 = v49 + 1;
    let v51 = v50 + 1;
    let v52 = v51 + 1;
    let v53 = v52 + 1;
    let v54 = v53 + 1;
    let v55 = v54 + 1;
    let v56 = v55 + 1;
    let v57 = v56 + 1;
    let v58 = v57 + 1;
    let v59 = v58 + 1;
    let v60 = v59 + 1;
    let v61 = v60 + 1;
    let v62 = v61 + 1;
    let v63 = v62 + 1;
    let v64 = v63 + 1;
    let v65 = v64 + 1;
    let v66 = v65 + 1;
    let v67 = v66 + 1;
    let v68 = v67 + 1;
    let v69 = v68 + 1;
    let v70 = v69 + 1;
    let v71 = v70 + 1;
    v71
}

// GOOD — a decision is as long as its cases, and the cases are not counted.
fn one_decision(at: u32) -> u32 {
    match at {
        0 => {
            0
        }
        1 => {
            1
        }
        2 => {
            2
        }
        3 => {
            3
        }
        4 => {
            4
        }
        5 => {
            5
        }
        6 => {
            6
        }
        7 => {
            7
        }
        8 => {
            8
        }
        9 => {
            9
        }
        10 => {
            10
        }
        11 => {
            11
        }
        12 => {
            12
        }
        13 => {
            13
        }
        14 => {
            14
        }
        15 => {
            15
        }
        16 => {
            16
        }
        17 => {
            17
        }
        18 => {
            18
        }
        19 => {
            19
        }
        20 => {
            20
        }
        21 => {
            21
        }
        22 => {
            22
        }
        23 => {
            23
        }
        24 => {
            24
        }
        25 => {
            25
        }
        26 => {
            26
        }
        27 => {
            27
        }
        28 => {
            28
        }
        29 => {
            29
        }
        30 => {
            30
        }
        31 => {
            31
        }
        32 => {
            32
        }
        33 => {
            33
        }
        34 => {
            34
        }
        35 => {
            35
        }
        36 => {
            36
        }
        37 => {
            37
        }
        38 => {
            38
        }
        39 => {
            39
        }
        40 => {
            40
        }
        41 => {
            41
        }
        42 => {
            42
        }
        43 => {
            43
        }
        44 => {
            44
        }
        45 => {
            45
        }
        46 => {
            46
        }
        47 => {
            47
        }
        48 => {
            48
        }
        49 => {
            49
        }
        50 => {
            50
        }
        51 => {
            51
        }
        52 => {
            52
        }
        53 => {
            53
        }
        54 => {
            54
        }
        55 => {
            55
        }
        56 => {
            56
        }
        57 => {
            57
        }
        58 => {
            58
        }
        59 => {
            59
        }
        60 => {
            60
        }
        61 => {
            61
        }
        62 => {
            62
        }
        63 => {
            63
        }
        64 => {
            64
        }
        65 => {
            65
        }
        66 => {
            66
        }
        67 => {
            67
        }
        68 => {
            68
        }
        69 => {
            69
        }
        70 => {
            70
        }
        71 => {
            71
        }
        72 => {
            72
        }
        73 => {
            73
        }
        74 => {
            74
        }
        75 => {
            75
        }
        76 => {
            76
        }
        77 => {
            77
        }
        78 => {
            78
        }
        79 => {
            79
        }
        _ => 0,
    }
}

// GOOD — blank lines are the room EXPLICIT013 asked for, and are not charged.
fn short(at: u32) -> u32 {
    let once = at + 1;

    let twice = once + 1;

    twice
}

fn main() {
    let _ = (long_walk(0), one_decision(1), short(2));
}
