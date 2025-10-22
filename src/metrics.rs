use std::sync::{Arc, Mutex};
use crossbeam_channel::Sender;

use crate::Event;

#[derive(Clone, Default)]
pub struct Registry {
    inner: Arc<Mutex<Inner>>,
}
#[derive(Default)]
struct Inner {
    ept_read: u64,
    ept_write: u64,
    ept_exec: u64,
    wx: u64,
    exits: std::collections::HashMap<String, u64>,
    by_vm: std::collections::HashMap<String, u64>,
}

impl Registry {
    pub fn new() -> Self { Self::default() }
    pub fn record(&self, ev: &Event) {
        let mut g = self.inner.lock().unwrap();
        if let Some(e) = &ev.ept {
            if e.read { g.ept_read += 1; }
            if e.write { g.ept_write += 1; }
            if e.exec { g.ept_exec += 1; }
        }
        *g.exits.entry(ev.reason.clone()).or_insert(0) += 1;
        *g.by_vm.entry(ev.vm.clone()).or_insert(0) += 1;
    }
    pub fn inc_wx(&self){ let mut g = self.inner.lock().unwrap(); g.wx += 1; }
    pub fn render(&self) -> String {
        let g = self.inner.lock().unwrap();
        let mut out = String::new();
        out.push_str("# HELP aegishv_ept_violations_total Count of EPT/NPT violations by type\n");
        out.push_str("# TYPE aegishv_ept_violations_total counter\n");
        out.push_str(&format!("aegishv_ept_violations_total{{type=\"read\"}} {}\n", g.ept_read));
        out.push_str(&format!("aegishv_ept_violations_total{{type=\"write\"}} {}\n", g.ept_write));
        out.push_str(&format!("aegishv_ept_violations_total{{type=\"exec\"}} {}\n", g.ept_exec));
        out.push_str("# HELP aegishv_wx_violation_total Write+Execute toggles on same GPA\n# TYPE aegishv_wx_violation_total counter\n");
        out.push_str(&format!("aegishv_wx_violation_total {}\n", g.wx));
        out.push_str("# HELP aegishv_vm_exits_total VM exits by reason\n# TYPE aegishv_vm_exits_total counter\n");
        for (k,v) in g.exits.iter() {
            out.push_str(&format!("aegishv_vm_exits_total{{reason=\"{}\"}} {}\n", k, v));
        }
        out
    }
}

pub fn emit_loop(reg: Registry, tx: Sender<String>){
    loop {
        let s = reg.render();
        let _ = tx.send(s);
        std::thread::sleep(std::time::Duration::from_millis(1000));
    }
}
