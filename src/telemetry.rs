// SPDX-License-Identifier: MIT
use crate::{
    platform::{cpu_seconds, Metrics},
    scene::Config,
};
use std::{
    fs::{self, File, OpenOptions},
    io::{self, BufWriter, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}
fn read(path: impl AsRef<Path>) -> String {
    fs::read_to_string(path)
        .unwrap_or_default()
        .trim()
        .to_owned()
}
pub fn environment() -> String {
    let mut power = Vec::new();
    if let Ok(entries) = fs::read_dir("/sys/class/power_supply") {
        for e in entries.flatten() {
            let p = e.path();
            power.push(format!(
                "{{\"type\":{},\"online\":{},\"status\":{}}}",
                quote(&read(p.join("type"))),
                quote(&read(p.join("online"))),
                quote(&read(p.join("status")))
            ));
        }
    }
    let mut temperatures = Vec::new();
    if let Ok(entries) = fs::read_dir("/sys/class/thermal") {
        for e in entries.flatten() {
            if e.file_name().to_string_lossy().starts_with("thermal_zone") {
                temperatures.push(format!(
                    "{{\"type\":{},\"millidegrees_c\":{}}}",
                    quote(&read(e.path().join("type"))),
                    quote(&read(e.path().join("temp")))
                ));
            }
        }
    }
    let status = read("/proc/self/status");
    let rss = status
        .lines()
        .find(|l| l.starts_with("VmRSS:"))
        .unwrap_or("");
    format!(
        "\"cpu_khz\":{},\"governor\":{},\"memory\":{},\"power\":[{}],\"temperatures\":[{}]",
        quote(&read(
            "/sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq"
        )),
        quote(&read(
            "/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor"
        )),
        quote(rss),
        power.join(","),
        temperatures.join(",")
    )
}
pub struct Log {
    writer: BufWriter<File>,
    directory: PathBuf,
}
impl Log {
    pub fn new(directory: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&directory)?;
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join("events.jsonl"))?;
        Ok(Self {
            writer: BufWriter::new(file),
            directory,
        })
    }
    pub fn event(&mut self, kind: &str, fields: &str) -> io::Result<()> {
        let at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        writeln!(
            self.writer,
            "{{\"schema\":1,\"unix_seconds\":{:.3},\"event\":{}{}{}}}",
            at,
            quote(kind),
            if fields.is_empty() { "" } else { "," },
            fields
        )?;
        self.writer.flush()
    }
    pub fn start(&mut self, width: u32, height: u32, capacity: usize, app: &str) -> io::Result<()> {
        let exe = std::env::current_exe()?;
        let root = exe
            .parent()
            .and_then(Path::parent)
            .unwrap_or(Path::new("."));
        let local = std::process::Command::new("date")
            .arg("--iso-8601=seconds")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .unwrap_or_default();
        self.event("start",&format!("\"application\":{},\"local_time\":{},\"source\":{},\"vendor_source\":{},\"driver_srcversion\":{},\"kernel\":{},\"width\":{},\"height\":{},\"vertex_capacity\":{},\"seed\":0,\"target_fps\":30,\"settle_seconds\":2,\"window_seconds\":10,\"lighting\":\"cpu_flat\",{}",quote(app),quote(&local),quote(&read(root.join("source-revision.txt"))),quote(&read(root.join("poulsback-revision.txt"))),quote(&read("/sys/module/poulsback/srcversion")),quote(&read("/proc/sys/kernel/osrelease")),width,height,capacity,environment()))
    }
    pub fn summary(&self, text: &str) -> io::Result<()> {
        let temp = self.directory.join("summary.new");
        let mut f = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        f.write_all(text.as_bytes())?;
        drop(f);
        fs::rename(temp, self.directory.join("summary.md"))
    }
}
#[derive(Clone, Copy)]
pub struct Sample {
    pub interval: f64,
    pub simulation: f64,
    pub geometry: f64,
    pub native: Metrics,
    pub vertices: usize,
}
pub struct Window {
    samples: Vec<Sample>,
    scratch: Vec<f64>,
    seconds: f64,
    cpu: f64,
}
impl Window {
    pub fn new() -> Self {
        Self {
            samples: Vec::with_capacity(1024),
            scratch: Vec::with_capacity(1024),
            seconds: 0.,
            cpu: 0.,
        }
    }
    pub fn push(&mut self, s: Sample) {
        if self.samples.is_empty() {
            self.cpu = cpu_seconds();
        }
        self.seconds += s.interval / 1000.;
        self.samples.push(s);
    }
    pub fn ready(&self) -> bool {
        self.seconds >= 10.
    }
    pub fn clear(&mut self) {
        self.samples.clear();
        self.seconds = 0.;
    }
    fn percentile(&mut self, f: impl Fn(&Sample) -> f64, p: f64) -> f64 {
        self.scratch.clear();
        self.scratch.extend(self.samples.iter().map(f));
        self.scratch.sort_unstable_by(|a, b| a.total_cmp(b));
        self.scratch[((self.scratch.len() as f64 * p).ceil() as usize)
            .saturating_sub(1)
            .min(self.scratch.len() - 1)]
    }
    pub fn report(&mut self, c: &Config) -> (bool, String, String) {
        let fps = self.samples.len() as f64 / self.seconds;
        let p50 = self.percentile(|s| s.interval, 0.5);
        let p95 = self.percentile(|s| s.interval, 0.95);
        let p99 = self.percentile(|s| s.interval, 0.99);
        let work = self.percentile(
            |s| {
                s.simulation
                    + s.geometry
                    + s.native.construct_ms
                    + s.native.submit_ms
                    + s.native.completion_ms
            },
            0.95,
        );
        let count = self.samples.len() as f64;
        let average = |f: fn(&Sample) -> f64| self.samples.iter().map(f).sum::<f64>() / count;
        let missed = self.samples.iter().filter(|s| s.interval > 35.).count();
        let max_vertices = self.samples.iter().map(|s| s.vertices).max().unwrap_or(0);
        let pass = fps >= 29.5 && p99 <= 35.;
        let fields=format!("{},\"pass\":{},\"frames\":{},\"seconds\":{:.3},\"fps\":{:.3},\"p50_ms\":{:.3},\"p95_ms\":{:.3},\"p99_ms\":{:.3},\"over_35ms\":{},\"process_cpu_percent\":{:.3},\"work_p95_ms\":{:.3},\"work_headroom_ms\":{:.3},\"simulation_ms\":{:.3},\"geometry_lighting_ms\":{:.3},\"construction_ms\":{:.3},\"submission_ms\":{:.3},\"completion_wait_ms\":{:.3},\"presentation_wait_ms\":{:.3},\"max_submitted_vertices\":{},\"max_submitted_triangles\":{},\"mean_draws\":{:.3},{}",c.json(),pass,count,self.seconds,fps,p50,p95,p99,missed,(cpu_seconds()-self.cpu)*100./self.seconds,work,33.333333-work,average(|s|s.simulation),average(|s|s.geometry),average(|s|s.native.construct_ms),average(|s|s.native.submit_ms),average(|s|s.native.completion_ms),average(|s|s.native.present_ms),max_vertices,max_vertices/3,average(|s|s.native.draws as f64),environment());
        let readable=format!("{} at level {}: {:.2} FPS; p99 {:.2} ms; {} frames over 35 ms.\nProcessing p95 {:.2} ms; {:.2} ms remaining in the 33.33 ms budget.\n",crate::scene::FAMILIES[c.family],c.level,fps,p99,missed,work,33.333333-work);
        (pass, fields, readable)
    }
}
