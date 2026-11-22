//! The one-dimensional inverse transforms of section 8.7.1, as the butterfly steps the
//! specification lists for each of them, generated from those lists by the decoder this came
//! from and carried here one step to a line.
//!
//! A step is a sum, a difference, a negation or a rotation by two of the specification's cosine
//! weights, and a rotation is rounded back to fourteen bits wherever the specification rounds
//! it. What a lane is is the caller's: one sample at thirty-two or sixty-four bits for a row, or
//! four or eight columns at once for the column pass, which is the same arithmetic per lane.
//!
//! A transform is a sequence of steps with no decision anywhere in it, and the longer ones are
//! longer than a screen. They are left whole: the stages the specification names are folded
//! together in the step lists, and cutting one of these anywhere but at a stage would be a cut
//! through a sequence nobody reads by hand.

use console_core_never::Never;

use crate::transform::{Lane, Weights};

#[inline(always)]
pub(crate) fn idct4<T: Lane>(x: [T; 4]) -> Result<[T; 4], Never> {
    let [x0, x1, x2, x3] = x;

    let Ok(v0) = x0.rotated(Weights { near: 11585, far: -11585 }, x2).and_then(Lane::rounded);
    let Ok(v1) = x0.rotated(Weights { near: 11585, far: 11585 }, x2).and_then(Lane::rounded);
    let Ok(v2) = x1.rotated(Weights { near: 6270, far: -15137 }, x3).and_then(Lane::rounded);
    let Ok(v3) = x1.rotated(Weights { near: 15137, far: 6270 }, x3).and_then(Lane::rounded);
    let Ok(v4) = v1.sum(v3);
    let Ok(v5) = v1.difference(v3);
    let Ok(v6) = v0.sum(v2);
    let Ok(v7) = v0.difference(v2);

    Ok([v4, v6, v7, v5])
}

#[inline(always)]
pub(crate) fn idct8<T: Lane>(x: [T; 8]) -> Result<[T; 8], Never> {
    let [x0, x1, x2, x3, x4, x5, x6, x7] = x;

    let Ok(v0) = x0.rotated(Weights { near: 11585, far: -11585 }, x4).and_then(Lane::rounded);
    let Ok(v1) = x0.rotated(Weights { near: 11585, far: 11585 }, x4).and_then(Lane::rounded);
    let Ok(v2) = x2.rotated(Weights { near: 6270, far: -15137 }, x6).and_then(Lane::rounded);
    let Ok(v3) = x2.rotated(Weights { near: 15137, far: 6270 }, x6).and_then(Lane::rounded);
    let Ok(v4) = v1.sum(v3);
    let Ok(v5) = v1.difference(v3);
    let Ok(v6) = v0.sum(v2);
    let Ok(v7) = v0.difference(v2);
    let Ok(v8) = x1.rotated(Weights { near: 3196, far: -16069 }, x7).and_then(Lane::rounded);
    let Ok(v9) = x1.rotated(Weights { near: 16069, far: 3196 }, x7).and_then(Lane::rounded);
    let Ok(v10) = x5.rotated(Weights { near: 13623, far: -9102 }, x3).and_then(Lane::rounded);
    let Ok(v11) = x5.rotated(Weights { near: 9102, far: 13623 }, x3).and_then(Lane::rounded);
    let Ok(v12) = v8.sum(v10);
    let Ok(v13) = v8.difference(v10);
    let Ok(v14) = v9.sum(v11);
    let Ok(v15) = v9.difference(v11);
    let Ok(v16) = v15.rotated(Weights { near: 11585, far: -11585 }, v13).and_then(Lane::rounded);
    let Ok(v17) = v15.rotated(Weights { near: 11585, far: 11585 }, v13).and_then(Lane::rounded);
    let Ok(v18) = v4.sum(v14);
    let Ok(v19) = v4.difference(v14);
    let Ok(v20) = v6.sum(v17);
    let Ok(v21) = v6.difference(v17);
    let Ok(v22) = v7.sum(v16);
    let Ok(v23) = v7.difference(v16);
    let Ok(v24) = v5.sum(v12);
    let Ok(v25) = v5.difference(v12);

    Ok([v18, v20, v22, v24, v25, v23, v21, v19])
}

#[cfg_attr(dylint_lib = "explicit050_no_long_function", allow(explicit050_no_long_function, reason = "the specification's butterfly steps for one transform, one to a line; the module head says why they are not cut"))]
#[inline(always)]
pub(crate) fn idct16<T: Lane>(x: [T; 16]) -> Result<[T; 16], Never> {
    let [x0, x1, x2, x3, x4, x5, x6, x7, x8, x9, x10, x11, x12, x13, x14, x15] = x;

    let Ok(v0) = x0.rotated(Weights { near: 11585, far: -11585 }, x8).and_then(Lane::rounded);
    let Ok(v1) = x0.rotated(Weights { near: 11585, far: 11585 }, x8).and_then(Lane::rounded);
    let Ok(v2) = x4.rotated(Weights { near: 6270, far: -15137 }, x12).and_then(Lane::rounded);
    let Ok(v3) = x4.rotated(Weights { near: 15137, far: 6270 }, x12).and_then(Lane::rounded);
    let Ok(v4) = v1.sum(v3);
    let Ok(v5) = v1.difference(v3);
    let Ok(v6) = v0.sum(v2);
    let Ok(v7) = v0.difference(v2);
    let Ok(v8) = x2.rotated(Weights { near: 3196, far: -16069 }, x14).and_then(Lane::rounded);
    let Ok(v9) = x2.rotated(Weights { near: 16069, far: 3196 }, x14).and_then(Lane::rounded);
    let Ok(v10) = x10.rotated(Weights { near: 13623, far: -9102 }, x6).and_then(Lane::rounded);
    let Ok(v11) = x10.rotated(Weights { near: 9102, far: 13623 }, x6).and_then(Lane::rounded);
    let Ok(v12) = v8.sum(v10);
    let Ok(v13) = v8.difference(v10);
    let Ok(v14) = v9.sum(v11);
    let Ok(v15) = v9.difference(v11);
    let Ok(v16) = v15.rotated(Weights { near: 11585, far: -11585 }, v13).and_then(Lane::rounded);
    let Ok(v17) = v15.rotated(Weights { near: 11585, far: 11585 }, v13).and_then(Lane::rounded);
    let Ok(v18) = v4.sum(v14);
    let Ok(v19) = v4.difference(v14);
    let Ok(v20) = v6.sum(v17);
    let Ok(v21) = v6.difference(v17);
    let Ok(v22) = v7.sum(v16);
    let Ok(v23) = v7.difference(v16);
    let Ok(v24) = v5.sum(v12);
    let Ok(v25) = v5.difference(v12);
    let Ok(v26) = x1.rotated(Weights { near: 1606, far: -16305 }, x15).and_then(Lane::rounded);
    let Ok(v27) = x1.rotated(Weights { near: 16305, far: 1606 }, x15).and_then(Lane::rounded);
    let Ok(v28) = x9.rotated(Weights { near: 12665, far: -10394 }, x7).and_then(Lane::rounded);
    let Ok(v29) = x9.rotated(Weights { near: 10394, far: 12665 }, x7).and_then(Lane::rounded);
    let Ok(v30) = x5.rotated(Weights { near: 7723, far: -14449 }, x11).and_then(Lane::rounded);
    let Ok(v31) = x5.rotated(Weights { near: 14449, far: 7723 }, x11).and_then(Lane::rounded);
    let Ok(v32) = x13.rotated(Weights { near: 15679, far: -4756 }, x3).and_then(Lane::rounded);
    let Ok(v33) = x13.rotated(Weights { near: 4756, far: 15679 }, x3).and_then(Lane::rounded);
    let Ok(v34) = v26.sum(v28);
    let Ok(v35) = v26.difference(v28);
    let Ok(v36) = v32.sum(v30);
    let Ok(v37) = v32.difference(v30);
    let Ok(v38) = v33.sum(v31);
    let Ok(v39) = v33.difference(v31);
    let Ok(v40) = v27.sum(v29);
    let Ok(v41) = v27.difference(v29);
    let Ok(v42) = v41.rotated(Weights { near: 6270, far: -15137 }, v35).and_then(Lane::rounded);
    let Ok(v43) = v41.rotated(Weights { near: 15137, far: 6270 }, v35).and_then(Lane::rounded);
    let Ok(v44) = v37.rotated(Weights { near: -15137, far: 6270 }, v39).and_then(Lane::rounded);
    let Ok(v45) = v37.rotated(Weights { near: -6270, far: -15137 }, v39).and_then(Lane::rounded);
    let Ok(v46) = v34.sum(v36);
    let Ok(v47) = v34.difference(v36);
    let Ok(v48) = v40.sum(v38);
    let Ok(v49) = v40.difference(v38);
    let Ok(v50) = v42.sum(v45);
    let Ok(v51) = v42.difference(v45);
    let Ok(v52) = v43.sum(v44);
    let Ok(v53) = v43.difference(v44);
    let Ok(v54) = v53.rotated(Weights { near: 11585, far: -11585 }, v51).and_then(Lane::rounded);
    let Ok(v55) = v53.rotated(Weights { near: 11585, far: 11585 }, v51).and_then(Lane::rounded);
    let Ok(v56) = v49.rotated(Weights { near: 11585, far: -11585 }, v47).and_then(Lane::rounded);
    let Ok(v57) = v49.rotated(Weights { near: 11585, far: 11585 }, v47).and_then(Lane::rounded);
    let Ok(v58) = v18.sum(v48);
    let Ok(v59) = v18.difference(v48);
    let Ok(v60) = v20.sum(v52);
    let Ok(v61) = v20.difference(v52);
    let Ok(v62) = v22.sum(v55);
    let Ok(v63) = v22.difference(v55);
    let Ok(v64) = v24.sum(v57);
    let Ok(v65) = v24.difference(v57);
    let Ok(v66) = v25.sum(v56);
    let Ok(v67) = v25.difference(v56);
    let Ok(v68) = v23.sum(v54);
    let Ok(v69) = v23.difference(v54);
    let Ok(v70) = v21.sum(v50);
    let Ok(v71) = v21.difference(v50);
    let Ok(v72) = v19.sum(v46);
    let Ok(v73) = v19.difference(v46);

    Ok([v58, v60, v62, v64, v66, v68, v70, v72, v73, v71, v69, v67, v65, v63, v61, v59])
}

#[cfg_attr(dylint_lib = "explicit050_no_long_function", allow(explicit050_no_long_function, reason = "the specification's butterfly steps for one transform, one to a line; the module head says why they are not cut"))]
#[inline(always)]
pub(crate) fn idct32<T: Lane>(x: [T; 32]) -> Result<[T; 32], Never> {
    let [x0, x1, x2, x3, x4, x5, x6, x7, x8, x9, x10, x11, x12, x13, x14, x15, x16, x17, x18, x19, x20, x21, x22, x23, x24, x25, x26, x27, x28, x29, x30, x31] = x;

    let Ok(v0) = x0.rotated(Weights { near: 11585, far: -11585 }, x16).and_then(Lane::rounded);
    let Ok(v1) = x0.rotated(Weights { near: 11585, far: 11585 }, x16).and_then(Lane::rounded);
    let Ok(v2) = x8.rotated(Weights { near: 6270, far: -15137 }, x24).and_then(Lane::rounded);
    let Ok(v3) = x8.rotated(Weights { near: 15137, far: 6270 }, x24).and_then(Lane::rounded);
    let Ok(v4) = v1.sum(v3);
    let Ok(v5) = v1.difference(v3);
    let Ok(v6) = v0.sum(v2);
    let Ok(v7) = v0.difference(v2);
    let Ok(v8) = x4.rotated(Weights { near: 3196, far: -16069 }, x28).and_then(Lane::rounded);
    let Ok(v9) = x4.rotated(Weights { near: 16069, far: 3196 }, x28).and_then(Lane::rounded);
    let Ok(v10) = x20.rotated(Weights { near: 13623, far: -9102 }, x12).and_then(Lane::rounded);
    let Ok(v11) = x20.rotated(Weights { near: 9102, far: 13623 }, x12).and_then(Lane::rounded);
    let Ok(v12) = v8.sum(v10);
    let Ok(v13) = v8.difference(v10);
    let Ok(v14) = v9.sum(v11);
    let Ok(v15) = v9.difference(v11);
    let Ok(v16) = v15.rotated(Weights { near: 11585, far: -11585 }, v13).and_then(Lane::rounded);
    let Ok(v17) = v15.rotated(Weights { near: 11585, far: 11585 }, v13).and_then(Lane::rounded);
    let Ok(v18) = v4.sum(v14);
    let Ok(v19) = v4.difference(v14);
    let Ok(v20) = v6.sum(v17);
    let Ok(v21) = v6.difference(v17);
    let Ok(v22) = v7.sum(v16);
    let Ok(v23) = v7.difference(v16);
    let Ok(v24) = v5.sum(v12);
    let Ok(v25) = v5.difference(v12);
    let Ok(v26) = x2.rotated(Weights { near: 1606, far: -16305 }, x30).and_then(Lane::rounded);
    let Ok(v27) = x2.rotated(Weights { near: 16305, far: 1606 }, x30).and_then(Lane::rounded);
    let Ok(v28) = x18.rotated(Weights { near: 12665, far: -10394 }, x14).and_then(Lane::rounded);
    let Ok(v29) = x18.rotated(Weights { near: 10394, far: 12665 }, x14).and_then(Lane::rounded);
    let Ok(v30) = x10.rotated(Weights { near: 7723, far: -14449 }, x22).and_then(Lane::rounded);
    let Ok(v31) = x10.rotated(Weights { near: 14449, far: 7723 }, x22).and_then(Lane::rounded);
    let Ok(v32) = x26.rotated(Weights { near: 15679, far: -4756 }, x6).and_then(Lane::rounded);
    let Ok(v33) = x26.rotated(Weights { near: 4756, far: 15679 }, x6).and_then(Lane::rounded);
    let Ok(v34) = v26.sum(v28);
    let Ok(v35) = v26.difference(v28);
    let Ok(v36) = v32.sum(v30);
    let Ok(v37) = v32.difference(v30);
    let Ok(v38) = v33.sum(v31);
    let Ok(v39) = v33.difference(v31);
    let Ok(v40) = v27.sum(v29);
    let Ok(v41) = v27.difference(v29);
    let Ok(v42) = v41.rotated(Weights { near: 6270, far: -15137 }, v35).and_then(Lane::rounded);
    let Ok(v43) = v41.rotated(Weights { near: 15137, far: 6270 }, v35).and_then(Lane::rounded);
    let Ok(v44) = v37.rotated(Weights { near: -15137, far: 6270 }, v39).and_then(Lane::rounded);
    let Ok(v45) = v37.rotated(Weights { near: -6270, far: -15137 }, v39).and_then(Lane::rounded);
    let Ok(v46) = v34.sum(v36);
    let Ok(v47) = v34.difference(v36);
    let Ok(v48) = v40.sum(v38);
    let Ok(v49) = v40.difference(v38);
    let Ok(v50) = v42.sum(v45);
    let Ok(v51) = v42.difference(v45);
    let Ok(v52) = v43.sum(v44);
    let Ok(v53) = v43.difference(v44);
    let Ok(v54) = v53.rotated(Weights { near: 11585, far: -11585 }, v51).and_then(Lane::rounded);
    let Ok(v55) = v53.rotated(Weights { near: 11585, far: 11585 }, v51).and_then(Lane::rounded);
    let Ok(v56) = v49.rotated(Weights { near: 11585, far: -11585 }, v47).and_then(Lane::rounded);
    let Ok(v57) = v49.rotated(Weights { near: 11585, far: 11585 }, v47).and_then(Lane::rounded);
    let Ok(v58) = v18.sum(v48);
    let Ok(v59) = v18.difference(v48);
    let Ok(v60) = v20.sum(v52);
    let Ok(v61) = v20.difference(v52);
    let Ok(v62) = v22.sum(v55);
    let Ok(v63) = v22.difference(v55);
    let Ok(v64) = v24.sum(v57);
    let Ok(v65) = v24.difference(v57);
    let Ok(v66) = v25.sum(v56);
    let Ok(v67) = v25.difference(v56);
    let Ok(v68) = v23.sum(v54);
    let Ok(v69) = v23.difference(v54);
    let Ok(v70) = v21.sum(v50);
    let Ok(v71) = v21.difference(v50);
    let Ok(v72) = v19.sum(v46);
    let Ok(v73) = v19.difference(v46);
    let Ok(v74) = x1.rotated(Weights { near: 804, far: -16364 }, x31).and_then(Lane::rounded);
    let Ok(v75) = x1.rotated(Weights { near: 16364, far: 804 }, x31).and_then(Lane::rounded);
    let Ok(v76) = x17.rotated(Weights { near: 12140, far: -11003 }, x15).and_then(Lane::rounded);
    let Ok(v77) = x17.rotated(Weights { near: 11003, far: 12140 }, x15).and_then(Lane::rounded);
    let Ok(v78) = x9.rotated(Weights { near: 7005, far: -14811 }, x23).and_then(Lane::rounded);
    let Ok(v79) = x9.rotated(Weights { near: 14811, far: 7005 }, x23).and_then(Lane::rounded);
    let Ok(v80) = x25.rotated(Weights { near: 15426, far: -5520 }, x7).and_then(Lane::rounded);
    let Ok(v81) = x25.rotated(Weights { near: 5520, far: 15426 }, x7).and_then(Lane::rounded);
    let Ok(v82) = x5.rotated(Weights { near: 3981, far: -15893 }, x27).and_then(Lane::rounded);
    let Ok(v83) = x5.rotated(Weights { near: 15893, far: 3981 }, x27).and_then(Lane::rounded);
    let Ok(v84) = x21.rotated(Weights { near: 14053, far: -8423 }, x11).and_then(Lane::rounded);
    let Ok(v85) = x21.rotated(Weights { near: 8423, far: 14053 }, x11).and_then(Lane::rounded);
    let Ok(v86) = x13.rotated(Weights { near: 9760, far: -13160 }, x19).and_then(Lane::rounded);
    let Ok(v87) = x13.rotated(Weights { near: 13160, far: 9760 }, x19).and_then(Lane::rounded);
    let Ok(v88) = x29.rotated(Weights { near: 16207, far: -2404 }, x3).and_then(Lane::rounded);
    let Ok(v89) = x29.rotated(Weights { near: 2404, far: 16207 }, x3).and_then(Lane::rounded);
    let Ok(v90) = v74.sum(v76);
    let Ok(v91) = v74.difference(v76);
    let Ok(v92) = v80.sum(v78);
    let Ok(v93) = v80.difference(v78);
    let Ok(v94) = v82.sum(v84);
    let Ok(v95) = v82.difference(v84);
    let Ok(v96) = v88.sum(v86);
    let Ok(v97) = v88.difference(v86);
    let Ok(v98) = v89.sum(v87);
    let Ok(v99) = v89.difference(v87);
    let Ok(v100) = v83.sum(v85);
    let Ok(v101) = v83.difference(v85);
    let Ok(v102) = v81.sum(v79);
    let Ok(v103) = v81.difference(v79);
    let Ok(v104) = v75.sum(v77);
    let Ok(v105) = v75.difference(v77);
    let Ok(v106) = v105.rotated(Weights { near: 3196, far: -16069 }, v91).and_then(Lane::rounded);
    let Ok(v107) = v105.rotated(Weights { near: 16069, far: 3196 }, v91).and_then(Lane::rounded);
    let Ok(v108) = v97.rotated(Weights { near: -9102, far: 13623 }, v99).and_then(Lane::rounded);
    let Ok(v109) = v97.rotated(Weights { near: -13623, far: -9102 }, v99).and_then(Lane::rounded);
    let Ok(v110) = v101.rotated(Weights { near: 13623, far: -9102 }, v95).and_then(Lane::rounded);
    let Ok(v111) = v101.rotated(Weights { near: 9102, far: 13623 }, v95).and_then(Lane::rounded);
    let Ok(v112) = v93.rotated(Weights { near: -16069, far: 3196 }, v103).and_then(Lane::rounded);
    let Ok(v113) = v93.rotated(Weights { near: -3196, far: -16069 }, v103).and_then(Lane::rounded);
    let Ok(v114) = v90.sum(v92);
    let Ok(v115) = v90.difference(v92);
    let Ok(v116) = v96.sum(v94);
    let Ok(v117) = v96.difference(v94);
    let Ok(v118) = v98.sum(v100);
    let Ok(v119) = v98.difference(v100);
    let Ok(v120) = v104.sum(v102);
    let Ok(v121) = v104.difference(v102);
    let Ok(v122) = v106.sum(v113);
    let Ok(v123) = v106.difference(v113);
    let Ok(v124) = v109.sum(v110);
    let Ok(v125) = v109.difference(v110);
    let Ok(v126) = v108.sum(v111);
    let Ok(v127) = v108.difference(v111);
    let Ok(v128) = v107.sum(v112);
    let Ok(v129) = v107.difference(v112);
    let Ok(v130) = v129.rotated(Weights { near: 6270, far: -15137 }, v123).and_then(Lane::rounded);
    let Ok(v131) = v129.rotated(Weights { near: 15137, far: 6270 }, v123).and_then(Lane::rounded);
    let Ok(v132) = v125.rotated(Weights { near: -15137, far: 6270 }, v127).and_then(Lane::rounded);
    let Ok(v133) = v125.rotated(Weights { near: -6270, far: -15137 }, v127).and_then(Lane::rounded);
    let Ok(v134) = v121.rotated(Weights { near: 6270, far: -15137 }, v115).and_then(Lane::rounded);
    let Ok(v135) = v121.rotated(Weights { near: 15137, far: 6270 }, v115).and_then(Lane::rounded);
    let Ok(v136) = v117.rotated(Weights { near: -15137, far: 6270 }, v119).and_then(Lane::rounded);
    let Ok(v137) = v117.rotated(Weights { near: -6270, far: -15137 }, v119).and_then(Lane::rounded);
    let Ok(v138) = v114.sum(v116);
    let Ok(v139) = v114.difference(v116);
    let Ok(v140) = v120.sum(v118);
    let Ok(v141) = v120.difference(v118);
    let Ok(v142) = v122.sum(v124);
    let Ok(v143) = v122.difference(v124);
    let Ok(v144) = v128.sum(v126);
    let Ok(v145) = v128.difference(v126);
    let Ok(v146) = v130.sum(v133);
    let Ok(v147) = v130.difference(v133);
    let Ok(v148) = v131.sum(v132);
    let Ok(v149) = v131.difference(v132);
    let Ok(v150) = v134.sum(v137);
    let Ok(v151) = v134.difference(v137);
    let Ok(v152) = v135.sum(v136);
    let Ok(v153) = v135.difference(v136);
    let Ok(v154) = v153.rotated(Weights { near: 11585, far: -11585 }, v151).and_then(Lane::rounded);
    let Ok(v155) = v153.rotated(Weights { near: 11585, far: 11585 }, v151).and_then(Lane::rounded);
    let Ok(v156) = v149.rotated(Weights { near: 11585, far: -11585 }, v147).and_then(Lane::rounded);
    let Ok(v157) = v149.rotated(Weights { near: 11585, far: 11585 }, v147).and_then(Lane::rounded);
    let Ok(v158) = v145.rotated(Weights { near: 11585, far: -11585 }, v143).and_then(Lane::rounded);
    let Ok(v159) = v145.rotated(Weights { near: 11585, far: 11585 }, v143).and_then(Lane::rounded);
    let Ok(v160) = v141.rotated(Weights { near: 11585, far: -11585 }, v139).and_then(Lane::rounded);
    let Ok(v161) = v141.rotated(Weights { near: 11585, far: 11585 }, v139).and_then(Lane::rounded);
    let Ok(v162) = v58.sum(v140);
    let Ok(v163) = v58.difference(v140);
    let Ok(v164) = v60.sum(v144);
    let Ok(v165) = v60.difference(v144);
    let Ok(v166) = v62.sum(v148);
    let Ok(v167) = v62.difference(v148);
    let Ok(v168) = v64.sum(v152);
    let Ok(v169) = v64.difference(v152);
    let Ok(v170) = v66.sum(v155);
    let Ok(v171) = v66.difference(v155);
    let Ok(v172) = v68.sum(v157);
    let Ok(v173) = v68.difference(v157);
    let Ok(v174) = v70.sum(v159);
    let Ok(v175) = v70.difference(v159);
    let Ok(v176) = v72.sum(v161);
    let Ok(v177) = v72.difference(v161);
    let Ok(v178) = v73.sum(v160);
    let Ok(v179) = v73.difference(v160);
    let Ok(v180) = v71.sum(v158);
    let Ok(v181) = v71.difference(v158);
    let Ok(v182) = v69.sum(v156);
    let Ok(v183) = v69.difference(v156);
    let Ok(v184) = v67.sum(v154);
    let Ok(v185) = v67.difference(v154);
    let Ok(v186) = v65.sum(v150);
    let Ok(v187) = v65.difference(v150);
    let Ok(v188) = v63.sum(v146);
    let Ok(v189) = v63.difference(v146);
    let Ok(v190) = v61.sum(v142);
    let Ok(v191) = v61.difference(v142);
    let Ok(v192) = v59.sum(v138);
    let Ok(v193) = v59.difference(v138);

    Ok([v162, v164, v166, v168, v170, v172, v174, v176, v178, v180, v182, v184, v186, v188, v190, v192, v193, v191, v189, v187, v185, v183, v181, v179, v177, v175, v173, v171, v169, v167, v165, v163])
}

#[inline(always)]
pub(crate) fn iadst8<T: Lane>(x: [T; 8]) -> Result<[T; 8], Never> {
    let [x0, x1, x2, x3, x4, x5, x6, x7] = x;

    let Ok(v0) = x7.rotated(Weights { near: 1606, far: -16305 }, x0);
    let Ok(v1) = x7.rotated(Weights { near: 16305, far: 1606 }, x0);
    let Ok(v2) = x5.rotated(Weights { near: 7723, far: -14449 }, x2);
    let Ok(v3) = x5.rotated(Weights { near: 14449, far: 7723 }, x2);
    let Ok(v4) = x3.rotated(Weights { near: 12665, far: -10394 }, x4);
    let Ok(v5) = x3.rotated(Weights { near: 10394, far: 12665 }, x4);
    let Ok(v6) = x1.rotated(Weights { near: 15679, far: -4756 }, x6);
    let Ok(v7) = x1.rotated(Weights { near: 4756, far: 15679 }, x6);
    let Ok(v8) = v1.sum(v5).and_then(Lane::rounded);
    let Ok(v9) = v1.difference(v5).and_then(Lane::rounded);
    let Ok(v10) = v0.sum(v4).and_then(Lane::rounded);
    let Ok(v11) = v0.difference(v4).and_then(Lane::rounded);
    let Ok(v12) = v3.sum(v7).and_then(Lane::rounded);
    let Ok(v13) = v3.difference(v7).and_then(Lane::rounded);
    let Ok(v14) = v2.sum(v6).and_then(Lane::rounded);
    let Ok(v15) = v2.difference(v6).and_then(Lane::rounded);
    let Ok(v16) = v9.rotated(Weights { near: 6270, far: -15137 }, v11);
    let Ok(v17) = v9.rotated(Weights { near: 15137, far: 6270 }, v11);
    let Ok(v18) = v15.rotated(Weights { near: 15137, far: -6270 }, v13);
    let Ok(v19) = v15.rotated(Weights { near: 6270, far: 15137 }, v13);
    let Ok(v20) = v17.sum(v18).and_then(Lane::rounded);
    let Ok(v21) = v17.difference(v18).and_then(Lane::rounded);
    let Ok(v22) = v16.sum(v19).and_then(Lane::rounded);
    let Ok(v23) = v16.difference(v19).and_then(Lane::rounded);
    let Ok(v24) = v8.sum(v12);
    let Ok(v25) = v8.difference(v12);
    let Ok(v26) = v10.sum(v14);
    let Ok(v27) = v10.difference(v14);
    let Ok(v28) = v25.rotated(Weights { near: 11585, far: -11585 }, v27).and_then(Lane::rounded);
    let Ok(v29) = v25.rotated(Weights { near: 11585, far: 11585 }, v27).and_then(Lane::rounded);
    let Ok(v30) = v21.rotated(Weights { near: 11585, far: -11585 }, v23).and_then(Lane::rounded);
    let Ok(v31) = v21.rotated(Weights { near: 11585, far: 11585 }, v23).and_then(Lane::rounded);
    let Ok(v32) = v20.negated();
    let Ok(v33) = v29.negated();
    let Ok(v34) = v30.negated();
    let Ok(v35) = v26.negated();

    Ok([v24, v32, v31, v33, v28, v34, v22, v35])
}

#[cfg_attr(dylint_lib = "explicit050_no_long_function", allow(explicit050_no_long_function, reason = "the specification's butterfly steps for one transform, one to a line; the module head says why they are not cut"))]
#[inline(always)]
pub(crate) fn iadst16<T: Lane>(x: [T; 16]) -> Result<[T; 16], Never> {
    let [x0, x1, x2, x3, x4, x5, x6, x7, x8, x9, x10, x11, x12, x13, x14, x15] = x;

    let Ok(v0) = x15.rotated(Weights { near: 804, far: -16364 }, x0);
    let Ok(v1) = x15.rotated(Weights { near: 16364, far: 804 }, x0);
    let Ok(v2) = x13.rotated(Weights { near: 3981, far: -15893 }, x2);
    let Ok(v3) = x13.rotated(Weights { near: 15893, far: 3981 }, x2);
    let Ok(v4) = x11.rotated(Weights { near: 7005, far: -14811 }, x4);
    let Ok(v5) = x11.rotated(Weights { near: 14811, far: 7005 }, x4);
    let Ok(v6) = x9.rotated(Weights { near: 9760, far: -13160 }, x6);
    let Ok(v7) = x9.rotated(Weights { near: 13160, far: 9760 }, x6);
    let Ok(v8) = x7.rotated(Weights { near: 12140, far: -11003 }, x8);
    let Ok(v9) = x7.rotated(Weights { near: 11003, far: 12140 }, x8);
    let Ok(v10) = x5.rotated(Weights { near: 14053, far: -8423 }, x10);
    let Ok(v11) = x5.rotated(Weights { near: 8423, far: 14053 }, x10);
    let Ok(v12) = x3.rotated(Weights { near: 15426, far: -5520 }, x12);
    let Ok(v13) = x3.rotated(Weights { near: 5520, far: 15426 }, x12);
    let Ok(v14) = x1.rotated(Weights { near: 16207, far: -2404 }, x14);
    let Ok(v15) = x1.rotated(Weights { near: 2404, far: 16207 }, x14);
    let Ok(v16) = v1.sum(v9).and_then(Lane::rounded);
    let Ok(v17) = v1.difference(v9).and_then(Lane::rounded);
    let Ok(v18) = v0.sum(v8).and_then(Lane::rounded);
    let Ok(v19) = v0.difference(v8).and_then(Lane::rounded);
    let Ok(v20) = v3.sum(v11).and_then(Lane::rounded);
    let Ok(v21) = v3.difference(v11).and_then(Lane::rounded);
    let Ok(v22) = v2.sum(v10).and_then(Lane::rounded);
    let Ok(v23) = v2.difference(v10).and_then(Lane::rounded);
    let Ok(v24) = v5.sum(v13).and_then(Lane::rounded);
    let Ok(v25) = v5.difference(v13).and_then(Lane::rounded);
    let Ok(v26) = v4.sum(v12).and_then(Lane::rounded);
    let Ok(v27) = v4.difference(v12).and_then(Lane::rounded);
    let Ok(v28) = v7.sum(v15).and_then(Lane::rounded);
    let Ok(v29) = v7.difference(v15).and_then(Lane::rounded);
    let Ok(v30) = v6.sum(v14).and_then(Lane::rounded);
    let Ok(v31) = v6.difference(v14).and_then(Lane::rounded);
    let Ok(v32) = v17.rotated(Weights { near: 3196, far: -16069 }, v19);
    let Ok(v33) = v17.rotated(Weights { near: 16069, far: 3196 }, v19);
    let Ok(v34) = v21.rotated(Weights { near: 13623, far: -9102 }, v23);
    let Ok(v35) = v21.rotated(Weights { near: 9102, far: 13623 }, v23);
    let Ok(v36) = v25.rotated(Weights { near: 16069, far: 3196 }, v27);
    let Ok(v37) = v25.rotated(Weights { near: -3196, far: 16069 }, v27);
    let Ok(v38) = v29.rotated(Weights { near: 9102, far: 13623 }, v31);
    let Ok(v39) = v29.rotated(Weights { near: -13623, far: 9102 }, v31);
    let Ok(v40) = v33.sum(v37).and_then(Lane::rounded);
    let Ok(v41) = v33.difference(v37).and_then(Lane::rounded);
    let Ok(v42) = v32.sum(v36).and_then(Lane::rounded);
    let Ok(v43) = v32.difference(v36).and_then(Lane::rounded);
    let Ok(v44) = v35.sum(v39).and_then(Lane::rounded);
    let Ok(v45) = v35.difference(v39).and_then(Lane::rounded);
    let Ok(v46) = v34.sum(v38).and_then(Lane::rounded);
    let Ok(v47) = v34.difference(v38).and_then(Lane::rounded);
    let Ok(v48) = v16.sum(v24);
    let Ok(v49) = v16.difference(v24);
    let Ok(v50) = v18.sum(v26);
    let Ok(v51) = v18.difference(v26);
    let Ok(v52) = v20.sum(v28);
    let Ok(v53) = v20.difference(v28);
    let Ok(v54) = v22.sum(v30);
    let Ok(v55) = v22.difference(v30);
    let Ok(v56) = v49.rotated(Weights { near: 6270, far: -15137 }, v51);
    let Ok(v57) = v49.rotated(Weights { near: 15137, far: 6270 }, v51);
    let Ok(v58) = v55.rotated(Weights { near: 15137, far: -6270 }, v53);
    let Ok(v59) = v55.rotated(Weights { near: 6270, far: 15137 }, v53);
    let Ok(v60) = v41.rotated(Weights { near: 6270, far: -15137 }, v43);
    let Ok(v61) = v41.rotated(Weights { near: 15137, far: 6270 }, v43);
    let Ok(v62) = v47.rotated(Weights { near: 15137, far: -6270 }, v45);
    let Ok(v63) = v47.rotated(Weights { near: 6270, far: 15137 }, v45);
    let Ok(v64) = v57.sum(v58).and_then(Lane::rounded);
    let Ok(v65) = v57.difference(v58).and_then(Lane::rounded);
    let Ok(v66) = v61.sum(v62).and_then(Lane::rounded);
    let Ok(v67) = v61.difference(v62).and_then(Lane::rounded);
    let Ok(v68) = v56.sum(v59).and_then(Lane::rounded);
    let Ok(v69) = v56.difference(v59).and_then(Lane::rounded);
    let Ok(v70) = v60.sum(v63).and_then(Lane::rounded);
    let Ok(v71) = v60.difference(v63).and_then(Lane::rounded);
    let Ok(v72) = v48.sum(v52);
    let Ok(v73) = v48.difference(v52);
    let Ok(v74) = v40.sum(v44);
    let Ok(v75) = v40.difference(v44);
    let Ok(v76) = v50.sum(v54);
    let Ok(v77) = v50.difference(v54);
    let Ok(v78) = v42.sum(v46);
    let Ok(v79) = v42.difference(v46);
    let Ok(v80) = v73.rotated(Weights { near: -11585, far: -11585 }, v77).and_then(Lane::rounded);
    let Ok(v81) = v73.rotated(Weights { near: 11585, far: -11585 }, v77).and_then(Lane::rounded);
    let Ok(v82) = v65.rotated(Weights { near: 11585, far: 11585 }, v69).and_then(Lane::rounded);
    let Ok(v83) = v65.rotated(Weights { near: -11585, far: 11585 }, v69).and_then(Lane::rounded);
    let Ok(v84) = v75.rotated(Weights { near: 11585, far: 11585 }, v79).and_then(Lane::rounded);
    let Ok(v85) = v75.rotated(Weights { near: -11585, far: 11585 }, v79).and_then(Lane::rounded);
    let Ok(v86) = v67.rotated(Weights { near: -11585, far: -11585 }, v71).and_then(Lane::rounded);
    let Ok(v87) = v67.rotated(Weights { near: 11585, far: -11585 }, v71).and_then(Lane::rounded);
    let Ok(v88) = v74.negated();
    let Ok(v89) = v70.negated();
    let Ok(v90) = v64.negated();
    let Ok(v91) = v76.negated();

    Ok([v72, v88, v66, v90, v82, v86, v84, v80, v81, v85, v87, v83, v68, v89, v78, v91])
}
