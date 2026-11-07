//! One Snippets only: the first to start owns a name on the session bus; a later one asks it to come forward and leaves.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use toolkit::toolkit::bus::{Asked, Bus, Offer, Props, Served};
use toolkit::toolkit::event::{Flow, Loop};

pub const NAME: &str = "org.trindle.Snippets";
const PATH: &str = "/org/trindle/Snippets";

fn wait(lp: &Loop, done: Rc<Cell<bool>>) {
    let (l, mut ticks) = (lp.clone(), 0);
    lp.every(Duration::from_millis(5), move || {
        ticks += 1;
        if done.get() || ticks > 200 {
            l.quit();
            return Flow::Stop;
        }
        Flow::Continue
    });
    lp.run();
}

pub fn is_first(bus: &Bus, lp: &Loop) -> bool {
    let (answered, first) = (Rc::new(Cell::new(false)), Rc::new(Cell::new(false)));
    let (a, f) = (answered.clone(), first.clone());
    bus.own_firmly(NAME, lp, move |ok| {
        f.set(ok);
        a.set(true);
    });
    wait(lp, answered);
    first.get()
}

pub fn serve(bus: &Bus, raise: impl Fn() + 'static) -> Option<Served> {
    let offer = Offer {
        name: NAME.into(),
        xml: format!("<interface name=\"{NAME}\"><method name=\"Raise\"/></interface>"),
        call: Box::new(move |a: Asked| match a.member().as_str() {
            "Raise" => {
                raise();
                a.reply(&());
            }
            other => a.fail("org.freedesktop.DBus.Error.UnknownMethod", other),
        }),
        props: Box::new(Props::new),
        set: None,
    };
    bus.serve(PATH, offer).ok()
}

pub fn ask_raise(bus: &Bus, lp: &Loop) -> bool {
    let (answered, ok) = (Rc::new(Cell::new(false)), Rc::new(Cell::new(false)));
    let (a, o) = (answered.clone(), ok.clone());
    bus.call(NAME, PATH, NAME).never_start().method("Raise", &(), false, Some(Duration::from_secs(1)), lp, move |r| {
        o.set(r.is_ok());
        a.set(true);
    });
    wait(lp, answered);
    ok.get()
}

pub fn another_answers(bus: &Bus, lp: &Loop) -> bool {
    !is_first(bus, lp) && ask_raise(bus, lp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use std::process::{Child, Command, Stdio};

    struct Daemon(Child, String);
    impl Drop for Daemon {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    fn daemon() -> Option<Daemon> {
        let spawned = Command::new("dbus-daemon").args(["--session", "--print-address=1", "--nofork", "--nopidfile"]).stdout(Stdio::piped()).stderr(Stdio::null()).spawn();
        let Ok(mut child) = spawned else {
            eprintln!("skipped: dbus-daemon is not installed");
            return None;
        };
        let mut line = String::new();
        BufReader::new(child.stdout.take().unwrap()).read_line(&mut line).unwrap();
        Some(Daemon(child, line.trim().to_string()))
    }

    #[test]
    fn a_second_snippets_brings_the_first_forward_and_does_not_open() {
        let Some(d) = daemon() else { return };
        let lp = Loop::new();
        let first = Bus::at(&lp, &d.1).expect("first bus");
        assert!(is_first(&first, &lp), "the first owns the name");
        let raised = Rc::new(Cell::new(0));
        let r = raised.clone();
        let _served = serve(&first, move || r.set(r.get() + 1)).expect("served");

        let second = Bus::at(&lp, &d.1).expect("second bus");
        assert!(!is_first(&second, &lp), "a second does not get the name");
        assert!(ask_raise(&second, &lp), "the first answers");
        assert_eq!(raised.get(), 1, "the first was asked to come forward once");
        let third = Bus::at(&lp, &d.1).expect("third bus");
        assert!(!is_first(&third, &lp));
        assert!(ask_raise(&third, &lp));
        assert_eq!(raised.get(), 2);
    }

    #[test]
    fn a_copy_that_gets_no_answer_opens_its_own_window() {
        let Some(d) = daemon() else { return };
        let lp = Loop::new();
        let first = Bus::at(&lp, &d.1).expect("first bus");
        assert!(is_first(&first, &lp));
        let second = Bus::at(&lp, &d.1).expect("second bus");
        assert!(!another_answers(&second, &lp), "the owner serves nothing, so it never answers");
        let _served = serve(&first, || {}).expect("served");
        let third = Bus::at(&lp, &d.1).expect("third bus");
        assert!(another_answers(&third, &lp));
    }
}
