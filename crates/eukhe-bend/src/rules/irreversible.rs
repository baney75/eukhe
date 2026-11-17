//! Effects outside the principal's grant and the ledger of the ones that
//! wait for the principal's approval (`bend/irreversible.bend`, entry
//! `bend/ffi_irreversible.bend`; `laws/irreversible.bend`, spec
//! `spec/irreversible/effects.bend`; spec/README.md I1-I6, I9): an effect
//! in the grant runs unasked; any other waits (the default mode) or is
//! asked (watch mode); a watch-mode question that closes unanswered leaves
//! it waiting; only an approval runs a waiting effect, once.

use eukhe_types::shell::{AutoSet, ShellEffects};

use crate::wire::{Reader, Writer};
use crate::{call, expect_core, Entry, Error};

/// The code of an effect of unknown kind: past the vocabulary of
/// [`eukhe_types::shell::ShellEffect`], so in no grant.
const UNKNOWN_CODE: u64 = 7;

/// What an effect declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Declared {
    /// Its effect classes (a `bash` step, a document access, a cell).
    Effects(ShellEffects),
    /// Nothing: a tool that declares no effects (an MCP tool not declared
    /// read-only). An effect of unknown kind is in no grant.
    Unknown,
}

/// Whether the principal watches the run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The default: nothing is asked; what needs an approval waits.
    End,
    /// Every effect outside the grant is asked.
    Watch,
}

/// What becomes of an effect when it is submitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// In the grant: it runs now, unasked.
    Runs,
    /// Recorded as waiting for an approval.
    Waits,
    /// Asked of the principal (watch mode).
    Asks,
}

/// How a question of watch mode ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Said {
    Yes,
    No,
    /// No answer: every client left, the deadline, a stop.
    Silent,
}

/// What the end of a question does to its effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fate {
    /// It runs.
    Runs,
    /// It does not run and does not wait.
    Refused,
    /// It waits for an approval.
    Waits,
}

/// The state of a ledger entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Held {
    Waiting,
    /// An approval took it: it ran, or runs.
    Taken,
}

/// One entry of the ledger: the effect's id and its state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LedgerEntry {
    pub id: u64,
    pub held: Held,
}

/// What an approval selects.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Which {
    All,
    Ids(Vec<u64>),
}

/// What an approval does: the ids to run, in ledger order, and the ledger
/// after it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Approved {
    pub run: Vec<u64>,
    pub ledger: Vec<LedgerEntry>,
}

/// What becomes of an effect that declares `declared` under the grant
/// `auto` in `mode` (`Ir.route`).
#[must_use]
pub fn route(auto: AutoSet, declared: Declared, mode: Mode) -> Route {
    expect_core(Entry::Irreversible, route_call(auto, declared, mode))
}

fn route_call(auto: AutoSet, declared: Declared, mode: Mode) -> Result<Route, Error> {
    let mut request = Writer::new();
    request.tag(0);
    let granted: Vec<u64> = auto
        .effects()
        .iter()
        .map(|effect| u64::from(effect.code()))
        .collect();
    nats(&mut request, &granted);
    let codes: Vec<u64> = match declared {
        Declared::Effects(effects) => effects
            .iter()
            .map(|effect| u64::from(effect.code()))
            .collect(),
        Declared::Unknown => vec![UNKNOWN_CODE],
    };
    nats(&mut request, &codes);
    request.tag(match mode {
        Mode::End => 0,
        Mode::Watch => 1,
    });
    let reply = call(Entry::Irreversible, &request.finish()?)?;
    let mut reader = Reader::reply(&reply)?;
    let route = match reader.tag_of(3, "a route")? {
        0 => Route::Runs,
        1 => Route::Waits,
        // `tag_of` answers a tag below 3: this is 2.
        _ => Route::Asks,
    };
    reader.end()?;
    Ok(route)
}

/// What the end `said` of a watch-mode question does (`Ir.settle`).
#[must_use]
pub fn settle(said: Said) -> Fate {
    expect_core(Entry::Irreversible, settle_call(said))
}

fn settle_call(said: Said) -> Result<Fate, Error> {
    let mut request = Writer::new();
    request.tag(1).tag(match said {
        Said::Yes => 0,
        Said::No => 1,
        Said::Silent => 2,
    });
    let reply = call(Entry::Irreversible, &request.finish()?)?;
    let mut reader = Reader::reply(&reply)?;
    let fate = match reader.tag_of(3, "a fate")? {
        0 => Fate::Runs,
        1 => Fate::Refused,
        // `tag_of` answers a tag below 3: this is 2.
        _ => Fate::Waits,
    };
    reader.end()?;
    Ok(fate)
}

/// The ledger with effect `id` recorded as waiting, last; an id it already
/// holds changes nothing (`Ir.hold`).
///
/// # Errors
///
/// An id or a ledger past the wire's bounds.
pub fn hold(ledger: &[LedgerEntry], id: u64) -> Result<Vec<LedgerEntry>, Error> {
    let mut request = Writer::new();
    request.tag(2);
    put_ledger(&mut request, ledger);
    request.nat(id);
    let reply = call(Entry::Irreversible, &request.finish()?)?;
    let mut reader = Reader::reply(&reply)?;
    let ledger = get_ledger(&mut reader)?;
    reader.end()?;
    Ok(ledger)
}

/// What the approval `which` runs and the ledger after it (`Ir.approve`).
///
/// # Errors
///
/// An id or a ledger past the wire's bounds.
pub fn approve(which: &Which, ledger: &[LedgerEntry]) -> Result<Approved, Error> {
    let mut request = Writer::new();
    request.tag(3);
    match which {
        Which::All => {
            request.tag(0);
        }
        Which::Ids(ids) => {
            request.tag(1);
            nats(&mut request, ids);
        }
    }
    put_ledger(&mut request, ledger);
    let reply = call(Entry::Irreversible, &request.finish()?)?;
    let mut reader = Reader::reply(&reply)?;
    let run = get_nats(&mut reader)?;
    let ledger = get_ledger(&mut reader)?;
    reader.end()?;
    Ok(Approved { run, ledger })
}

/// The waiting ids, in recorded order: the end-of-run list
/// (`Ir.waiting`).
///
/// # Errors
///
/// An id or a ledger past the wire's bounds.
pub fn waiting(ledger: &[LedgerEntry]) -> Result<Vec<u64>, Error> {
    let mut request = Writer::new();
    request.tag(4);
    put_ledger(&mut request, ledger);
    let reply = call(Entry::Irreversible, &request.finish()?)?;
    let mut reader = Reader::reply(&reply)?;
    let ids = get_nats(&mut reader)?;
    reader.end()?;
    Ok(ids)
}

fn nats(request: &mut Writer, values: &[u64]) {
    request.count(values.len());
    for value in values {
        request.nat(*value);
    }
}

fn get_nats(reader: &mut Reader<'_>) -> Result<Vec<u64>, Error> {
    let count = reader.count()?;
    (0..count)
        .map(|_| reader.nat().map_err(Error::from))
        .collect()
}

/// A ledger is one list of Nats: each entry's id, then its state.
fn put_ledger(request: &mut Writer, ledger: &[LedgerEntry]) {
    request.count(ledger.len() * 2);
    for entry in ledger {
        request.nat(entry.id).nat(match entry.held {
            Held::Waiting => 0,
            Held::Taken => 1,
        });
    }
}

fn get_ledger(reader: &mut Reader<'_>) -> Result<Vec<LedgerEntry>, Error> {
    let pairs = reader.count()? / 2;
    (0..pairs)
        .map(|_| {
            let id = reader.nat()?;
            let held = match reader.tag_of(2, "a ledger state")? {
                0 => Held::Waiting,
                // `tag_of` answers a tag below 2: this is 1.
                _ => Held::Taken,
            };
            Ok(LedgerEntry { id, held })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use eukhe_types::shell::{AutoSet, ShellEffect, ShellEffects};

    use super::*;

    fn waiting_entry(id: u64) -> LedgerEntry {
        LedgerEntry {
            id,
            held: Held::Waiting,
        }
    }

    /// The shipped grant runs an edit and holds a push; watch mode asks;
    /// an effect of unknown kind is in no grant, the widest included.
    #[test]
    fn the_core_routes_by_the_grant_and_the_mode() {
        let shipped = AutoSet::new([ShellEffect::Read, ShellEffect::Write, ShellEffect::Exec]);
        let every = AutoSet::new(ShellEffect::ALL);
        let edit = Declared::Effects(ShellEffects::of(&[ShellEffect::Write]));
        let push = Declared::Effects(ShellEffects::of(&[ShellEffect::Exec, ShellEffect::Net]));
        assert_eq!(
            [
                route(shipped, edit, Mode::End),
                route(shipped, push, Mode::End),
                route(shipped, push, Mode::Watch),
                route(every, push, Mode::End),
                route(every, Declared::Unknown, Mode::End),
                route(every, Declared::Unknown, Mode::Watch),
            ],
            [
                Route::Runs,
                Route::Waits,
                Route::Asks,
                Route::Runs,
                Route::Waits,
                Route::Asks
            ]
        );
    }

    /// Silence is never a yes: no answer leaves the effect waiting.
    #[test]
    fn the_core_settles_a_question() {
        assert_eq!(
            [settle(Said::Yes), settle(Said::No), settle(Said::Silent)],
            [Fate::Runs, Fate::Refused, Fate::Waits]
        );
    }

    /// The ledger round trip: two effects held, one approved by id, then
    /// all: each runs once, in order, and nothing is left waiting.
    #[test]
    fn the_core_runs_each_approved_effect_once() -> Result<(), Error> {
        let ledger = hold(&hold(&[], 1)?, 2)?;
        assert_eq!(ledger, vec![waiting_entry(1), waiting_entry(2)]);
        assert_eq!(hold(&ledger, 1)?, ledger);
        assert_eq!(waiting(&ledger)?, vec![1, 2]);
        let first = approve(&Which::Ids(vec![2]), &ledger)?;
        assert_eq!(
            first,
            Approved {
                run: vec![2],
                ledger: vec![
                    waiting_entry(1),
                    LedgerEntry {
                        id: 2,
                        held: Held::Taken
                    }
                ],
            }
        );
        let rest = approve(&Which::All, &first.ledger)?;
        assert_eq!(rest.run, vec![1]);
        assert_eq!(approve(&Which::All, &rest.ledger)?.run, Vec::<u64>::new());
        assert_eq!(waiting(&rest.ledger)?, Vec::<u64>::new());
        Ok(())
    }
}
