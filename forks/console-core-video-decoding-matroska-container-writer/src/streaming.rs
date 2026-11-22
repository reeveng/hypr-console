/// One demuxed frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Packet {
    /// Index into [`MkvFile::tracks`].
    pub track: usize,
    /// Matroska `TrackNumber`.
    pub track_number: u64,
    /// Presentation timestamp in timestamp ticks (the track timebase, [`Track::timebase`]).
    pub pts: i64,
    /// Presentation timestamp in nanoseconds.
    pub pts_ns: i64,
    /// Duration in ticks (0 if unknown).
    pub duration: u64,
    /// Duration in nanoseconds (0 if unknown).
    pub duration_ns: u64,
    pub keyframe: bool,
    pub discardable: bool,
    pub invisible: bool,
    /// `DiscardPadding` (ns) from the enclosing BlockGroup.
    pub discard_padding_ns: Option<i64>,
    /// Absolute offset of the stored frame data.
    pub offset: u64,
    /// Index of the frame within a laced block (0 when not laced). Laced frames after the first
    /// only have exact timestamps when the block or track carries a duration; otherwise they
    /// repeat the block timestamp and `duration` is 0.
    pub lace: u16,
    /// Frame bytes with header stripping undone. Frames of tracks with other content encodings
    /// (zlib, encryption) are returned as stored; see [`Track::frames_readable`].
    pub data: Vec<u8>,
}

/// Result of [`Demuxer::seek`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeekPoint {
    pub track: usize,
    /// Keyframe timestamp in ticks and ns.
    pub pts: i64,
    pub pts_ns: i64,
    /// Sample index of the keyframe when the file is indexed.
    pub sample: Option<usize>,
    pub cluster_offset: u64,
    pub block_offset: u64,
}

/// A keyframe index entry ([`Demuxer::keyframe_index`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Keyframe {
    pub sample: usize,
    pub pts: i64,
    pub pts_ns: i64,
    pub block_offset: u64,
}

// ---------------------------------------------------------------------------------------------
// Demuxer (streaming packets + seeking)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct InCluster {
    ts: i64,
    end: Option<u64>,
}

/// Packet reader over a [`ByteSource`]: iterates frames in file order and seeks to keyframes.
pub struct Demuxer<S> {
    src: S,
    file: MkvFile,
    opts: OpenOptions,
    cache: Cache,
    pos: u64,
    cluster: Option<InCluster>,
    pending: VecDeque<FrameRef>,
    skip_laces: Option<(u64, u16)>,
    done: bool,
}

impl<R: std::io::Read + std::io::Seek> Demuxer<ReadSeekSource<R>> {
    /// Open from any `Read + Seek` stream.
    pub fn from_reader(r: R) -> Result<Self> {
        Self::new(ReadSeekSource::new(r)?)
    }
}

impl<'a> Demuxer<&'a [u8]> {
    /// Open from an in-memory byte slice.
    pub fn from_slice(data: &'a [u8]) -> Result<Self> {
        Self::new(data)
    }
}

impl<S: ByteSource> Demuxer<S> {
    /// Open with default options (full index).
    pub fn new(src: S) -> Result<Self> {
        Self::with_options(src, OpenOptions::default())
    }

    pub fn with_options(src: S, opts: OpenOptions) -> Result<Self> {
        let mut cache = Cache::default();
        let file = open_impl(&src, &opts, &mut cache)?;
        let pos = file.first_cluster.unwrap_or(file.segment_end);
        Ok(Self { src, file, opts, cache, pos, cluster: None, pending: VecDeque::new(), skip_laces: None, done: false })
    }

    pub fn file(&self) -> &MkvFile {
        &self.file
    }
    pub fn tracks(&self) -> &[Track] {
        &self.file.tracks
    }
    pub fn source(&self) -> &S {
        &self.src
    }
    pub fn into_parts(self) -> (S, MkvFile) {
        (self.src, self.file)
    }
    /// Segment duration in ns.
    pub fn duration_ns(&self) -> Option<u64> {
        self.file.duration_ns()
    }

    /// Scan all clusters and build sample tables (no-op if already indexed).
    pub fn build_index(&mut self) -> Result<()> {
        if self.file.indexed {
            return Ok(());
        }
        let opts = OpenOptions { index: true, verify_crc: self.opts.verify_crc };
        self.file = open_impl(&self.src, &opts, &mut self.cache)?;
        Ok(())
    }

    /// Keyframes of `track` (builds the index if needed).
    pub fn keyframe_index(&mut self, track: usize) -> Result<Vec<Keyframe>> {
        self.build_index()?;
        let t = self.file.tracks.get(track).ok_or(Error::NoSuchTrack(track))?;
        Ok(t.keyframes()
            .iter()
            .map(|&k| {
                let s = &t.samples[k as usize];
                Keyframe { sample: k as usize, pts: s.pts, pts_ns: t.ticks_to_ns(s.pts), block_offset: s.block_offset }
            })
            .collect())
    }

    /// Rewind to the first cluster.
    pub fn rewind(&mut self) {
        self.pos = self.file.first_cluster.unwrap_or(self.file.segment_end);
        self.cluster = None;
        self.pending.clear();
        self.skip_laces = None;
        self.done = false;
    }

    /// Read sample `index` of `track` (requires an index).
    pub fn read_sample(&self, track: usize, index: usize) -> Result<Vec<u8>> {
        self.file.read_sample(&self.src, track, index)
    }

    /// Position the packet stream at the keyframe of `track` with the greatest pts ≤ `time_ns`
    /// (or the track's first keyframe if none precedes it). The next [`Demuxer::next_packet`]
    /// call returns packets starting at that keyframe's block (all tracks).
    ///
    /// Uses the sample index when present, else Cues (refined by scanning the cued cluster), else
    /// builds the index first.
    pub fn seek(&mut self, track: usize, time_ns: i64) -> Result<SeekPoint> {
        let t = self.file.tracks.get(track).ok_or(Error::NoSuchTrack(track))?;
        let ticks = t.ns_to_ticks(time_ns);
        if !self.file.indexed {
            let tn = t.number;
            let cue = self
                .file
                .cues
                .iter()
                .filter(|c| c.time as i64 <= ticks)
                .filter_map(|c| c.positions.iter().find(|p| p.track == tn).map(|p| (c.time, *p)))
                .next_back()
                .or_else(|| self.file.cues.iter().find_map(|c| c.positions.iter().find(|p| p.track == tn).map(|p| (c.time, *p))));
            if let Some((_, cp)) = cue
                && let Some(sp) = self.seek_via_cue(track, ticks, cp.cluster_position)?
            {
                return Ok(sp);
            }
            self.build_index()?;
        }
        let t = &self.file.tracks[track];
        let k = t.keyframe_before_pts(ticks).ok_or_else(|| Error::Invalid(format!("track {} has no keyframes", t.number)))?;
        let s = t.samples[k];
        let c = self.file.clusters[s.cluster as usize];
        let sp = SeekPoint { track, pts: s.pts, pts_ns: t.ticks_to_ns(s.pts), sample: Some(k), cluster_offset: c.offset, block_offset: s.block_offset };
        self.position_at(c, s.block_offset, s.lace);
        Ok(sp)
    }

    fn position_at(&mut self, c: ClusterInfo, block_offset: u64, lace: u16) {
        self.pending.clear();
        self.done = false;
        self.pos = block_offset;
        let known = c.end;
        self.cluster = Some(InCluster { ts: c.timestamp as i64, end: Some(known) });
        self.skip_laces = (lace > 0).then_some((block_offset, lace));
    }

    fn seek_via_cue(&mut self, track: usize, ticks: i64, cluster_position: u64) -> Result<Option<SeekPoint>> {
        let abs = self.file.segment_data_start + cluster_position;
        let mut io = Io::new(&self.src, &mut self.cache)?;
        let Some(h) = io.header(abs, self.file.segment_end)? else { return Ok(None) };
        if h.id != CLUSTER {
            self.file.warnings.push(format!("cue points at {abs}, which is not a Cluster"));
            return Ok(None);
        }
        let mut warnings = Vec::new();
        let (ci, blocks) = scan_cluster(&mut io, &h, self.file.segment_end, &self.file.tracks, &mut warnings)?;
        let t = &self.file.tracks[track];
        let mut frames = Vec::new();
        for b in blocks.iter().filter(|b| b.track == track && b.keyframe) {
            expand(b, t, self.file.info.timestamp_scale, 0, &mut frames);
        }
        let best = frames.iter().filter(|f| f.pts <= ticks).max_by_key(|f| f.pts).or_else(|| frames.iter().min_by_key(|f| f.pts)).copied();
        let Some(f) = best else { return Ok(None) };
        let sp = SeekPoint { track, pts: f.pts, pts_ns: f.pts_ns, sample: None, cluster_offset: ci.offset, block_offset: f.block_offset };
        self.position_at(ci, f.block_offset, f.lace);
        Ok(Some(sp))
    }

    /// Next frame in file order, or `None` at the end of the Segment.
    pub fn next_packet(&mut self) -> Result<Option<Packet>> {
        loop {
            if let Some(f) = self.pending.pop_front() {
                return self.materialize(f).map(Some);
            }
            if self.done || !self.fill()? {
                self.done = true;
                return Ok(None);
            }
        }
    }

    fn materialize(&mut self, f: FrameRef) -> Result<Packet> {
        let t = &self.file.tracks[f.track];
        let mut data = if t.frames_readable() { t.frame_prefix(f.size as u64) } else { Vec::new() };
        let at = data.len();
        data.resize(at + f.size as usize, 0);
        let mut io = Io::new(&self.src, &mut self.cache)?;
        if (f.size as usize) <= WIN / 2 {
            let b = io.peek(f.offset, f.size as usize)?;
            if b.len() != f.size as usize {
                return Err(Error::Invalid("frame data past end of file".into()));
            }
            data[at..].copy_from_slice(b);
        } else {
            self.src.read_at(f.offset, &mut data[at..])?;
        }
        Ok(Packet {
            track: f.track,
            track_number: t.number,
            pts: f.pts,
            pts_ns: f.pts_ns,
            duration: f.duration,
            duration_ns: f.duration_ns,
            keyframe: f.keyframe,
            discardable: f.discardable,
            invisible: f.invisible,
            discard_padding_ns: f.discard_padding,
            offset: f.offset,
            lace: f.lace,
            data,
        })
    }

    /// Advance over elements until at least one frame is pending. Returns false at the end.
    fn fill(&mut self) -> Result<bool> {
        let seg_end = self.file.segment_end;
        let scale = self.file.info.timestamp_scale;
        let mut io = Io::new(&self.src, &mut self.cache)?;
        let lookup = track_lookup(&self.file.tracks);
        loop {
            if let Some(c) = self.cluster {
                let lim = c.end.unwrap_or(seg_end);
                if self.pos >= lim {
                    self.cluster = None;
                    continue;
                }
                let Some(h) = io.header(self.pos, lim)? else {
                    self.cluster = None;
                    match io.resync(self.pos + 1, seg_end)? {
                        Some(p) => {
                            self.pos = p;
                            continue;
                        }
                        None => return Ok(false),
                    }
                };
                if c.end.is_none() && is_top_level(h.id) {
                    self.cluster = None;
                    continue;
                }
                if h.size == Size::Unknown || h.overruns(lim) {
                    self.cluster = None;
                    match io.resync(self.pos + 1, seg_end)? {
                        Some(p) => {
                            self.pos = p;
                            continue;
                        }
                        None => return Ok(false),
                    }
                }
                let he = h.end(lim);
                self.pos = he;
                match h.id {
                    TIMESTAMP => {
                        let v = ebml::uint(io.peek(h.data_start(), (he - h.data_start()) as usize)?) as i64;
                        self.cluster = Some(InCluster { ts: v, end: c.end });
                    }
                    SIMPLE_BLOCK | BLOCK_GROUP => {
                        if let Some(b) = parse_block_elem(&mut io, &h, he, c.ts, &lookup)? {
                            let skip = match self.skip_laces.take() {
                                Some((off, n)) if off == h.pos => n,
                                _ => 0,
                            };
                            expand(&b, &self.file.tracks[b.track], scale, skip, &mut self.pending);
                            if !self.pending.is_empty() {
                                return Ok(true);
                            }
                        }
                    }
                    _ => {}
                }
                continue;
            }
            if self.pos >= seg_end {
                return Ok(false);
            }
            let Some(h) = io.header(self.pos, seg_end)? else {
                match io.resync(self.pos + 1, seg_end)? {
                    Some(p) => {
                        self.pos = p;
                        continue;
                    }
                    None => return Ok(false),
                }
            };
            match h.id {
                CLUSTER => {
                    self.cluster = Some(InCluster { ts: 0, end: h.size.known().map(|_| h.end(seg_end)) });
                    self.pos = h.data_start();
                }
                EBML | SEGMENT => return Ok(false),
                _ => {
                    self.pos = if h.size == Size::Unknown { io.unknown_end(h.data_start(), seg_end)? } else { h.end(seg_end) };
                    if self.pos <= h.pos {
                        return Ok(false);
                    }
                }
            }
        }
    }
}

impl<S: ByteSource> Iterator for Demuxer<S> {
    type Item = Result<Packet>;
    fn next(&mut self) -> Option<Result<Packet>> {
        match self.next_packet() {
            Ok(Some(p)) => Some(Ok(p)),
            Ok(None) => None,
            Err(e) => {
                self.done = true;
                Some(Err(e))
            }
        }
    }
}
