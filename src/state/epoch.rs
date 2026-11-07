#[derive(Debug, Default)]
pub struct Epoch(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ticket(u64);

impl Epoch {
    pub fn next(&mut self) -> Ticket {
        self.0 += 1;
        Ticket(self.0)
    }

    pub fn current(&self) -> Ticket {
        Ticket(self.0)
    }

    pub fn is_current(&self, t: Ticket) -> bool {
        t.0 == self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_ticket_invalidates_the_old_one() {
        let mut e = Epoch::default();
        let a = e.next();
        assert!(e.is_current(a));
        let b = e.next();
        assert!(!e.is_current(a));
        assert!(e.is_current(b));
        assert_eq!(e.current(), b);
    }
}
