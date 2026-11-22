impl MasteringDisplay {
    /// BT.2020 primaries, D65, with the given peak and black in cd/m².
    pub fn bt2020(max_nits: f64, min_nits: f64) -> Self {
        let q = |v: f64| (v / 0.00002).round() as u16;
        MasteringDisplay {
            primaries: [(q(0.170), q(0.797)), (q(0.131), q(0.046)), (q(0.708), q(0.292))],
            white_point: (q(0.3127), q(0.3290)),
            max_luminance: (max_nits * 10_000.0).round() as u32,
            min_luminance: (min_nits * 10_000.0).round() as u32,
        }
    }
    pub fn max_nits(&self) -> f64 {
        self.max_luminance as f64 / 10_000.0
    }
    /// The 24-byte payload shared by `mdcv` and the SEI message.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(24);
        for (x, y) in self.primaries {
            v.extend_from_slice(&x.to_be_bytes());
            v.extend_from_slice(&y.to_be_bytes());
        }
        v.extend_from_slice(&self.white_point.0.to_be_bytes());
        v.extend_from_slice(&self.white_point.1.to_be_bytes());
        v.extend_from_slice(&self.max_luminance.to_be_bytes());
        v.extend_from_slice(&self.min_luminance.to_be_bytes());
        v
    }
}

impl AvcConfig {
    /// Build from parameter sets (NAL units without start codes); profile/level taken from the first SPS.
    pub fn new(sps: Vec<Vec<u8>>, pps: Vec<Vec<u8>>, length_size: u8) -> Self {
        let (profile, compatibility, level) = sps.first().filter(|s| s.len() >= 4).map(|s| (s[1], s[2], s[3])).unwrap_or((0, 0, 0));
        AvcConfig { profile, compatibility, level, length_size, sps, pps, ext: Vec::new() }
    }
}

impl Av1Config {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = BoxBuf::new();
        b.u8(0x81);
        b.u8((self.seq_profile << 5) | (self.seq_level_idx_0 & 0x1F));
        b.u8(((self.seq_tier_0 as u8) << 7)
            | ((self.high_bitdepth as u8) << 6)
            | ((self.twelve_bit as u8) << 5)
            | ((self.monochrome as u8) << 4)
            | ((self.chroma_subsampling_x as u8) << 3)
            | ((self.chroma_subsampling_y as u8) << 2)
            | (self.chroma_sample_position & 3));
        b.u8(match self.initial_presentation_delay_minus_one {
            Some(d) => 0x10 | (d & 0x0F),
            None => 0,
        });
        b.bytes(&self.config_obus);
        b.buf
    }
}

/// `vpcC` — VP codec configuration (version 1).
}

impl VpcConfig {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = BoxBuf::new();
        b.u32(1 << 24);
        b.u8(self.profile);
        b.u8(self.level);
        b.u8((self.bit_depth << 4) | ((self.chroma_subsampling & 7) << 1) | self.full_range as u8);
        b.u8(self.colour_primaries);
        b.u8(self.transfer_characteristics);
        b.u8(self.matrix_coefficients);
        b.u16(self.codec_init.len() as u16);
        b.bytes(&self.codec_init);
        b.buf
    }
}

}

impl OpusConfig {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = BoxBuf::new();
        b.u8(0);
        b.u8(self.output_channels);
        b.u16(self.pre_skip);
        b.u32(self.input_sample_rate);
        b.i16(self.output_gain);
        b.u8(self.channel_mapping_family);
        b.bytes(&self.channel_mapping);
        b.buf
    }
}

}

impl ApvConfig {
    /// Serialize the full `apvC` box payload (4-byte `FullBox` header + `APVDecoderConfigurationRecord`).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = BoxBuf::new();
        b.u32(0); // FullBox version = 0, flags = 0
        if !self.raw_record.is_empty() {
            b.bytes(&self.raw_record);
            return b.buf;
        }
        b.u8(1); // configurationVersion = 1
        b.u8(1); // number_of_configuration_entry = 1
        b.u8(1); // pbu_type = 1 (primary frame)
        b.u8(1); // number_of_frame_info = 1
        let flags = (if self.color_description_present { 0x02 } else { 0 }) | (if self.capture_time_distance_ignored { 0x01 } else { 0 });
        b.u8(flags);
        b.u8(self.profile_idc);
        b.u8(self.level_idc);
        b.u8(self.band_idc);
        b.u32(self.frame_width);
        b.u32(self.frame_height);
        b.u8((self.chroma_format_idc << 4) | (self.bit_depth_minus8 & 0x0F));
        b.u8(self.capture_time_distance);
        if self.color_description_present {
            b.u8(self.color_primaries);
            b.u8(self.transfer_characteristics);
            b.u8(self.matrix_coefficients);
            b.u8(if self.full_range { 0x80 } else { 0 });
        }
        b.buf
    }
}
