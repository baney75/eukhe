# approvals: the model of where a question of the approval gate goes in
# watch mode (crates/eukhe-daemon/src/shell_approval.rs; spec/README.md
# I5, I9).
#
# A `bash` step or an `python` cell runs unasked when its effects are
# within the grant (bend/tools.bend `within`); the shipped grant is read,
# write and exec (spec/README.md I1). Any other step, outside watch mode,
# waits for the principal's approval (bend/irreversible.bend) and nothing
# is asked. In watch mode it is asked. A chain is the sessions from the one
# that asks up to its root: the asking session first, then its parent, and
# so on. Each session, when the question reaches it, is one of:
#
#   Away{}       no client is attached: the session hands the question to
#                its parent at once (`relay_shell_approval`).
#   Left{}       its clients get the question (a `shell_approval` frame),
#                and all of them detach without an answer: the session
#                hands the question to its parent then.
#   Says{yes}    its clients get the question, and the first answer
#                approves (True) or denies (False).
#
# The clients are the TUI, the dashboard and the phone gateway, which asks
# over Signal and sends the phone's answer. The question goes up the
# chain until a session's clients answer; above the root there is no one,
# and the question is unanswered: the step waits for an approval
# (bend/irreversible.bend `settle`). A question no one answers is never a
# yes. A frame on a session above the asker names the subagent that asks.
# The effects are their codes (bend/tools.bend `Eff.code`).

import Base
import ./tools.bend as Tl

type Desk is Data:
  Away{}
  Left{}
  Says{yes: Bool}

# How a question ended: the clients of session `at` (0: the asker, 1: its
# parent, ...) answered, or no one did
type End is Data:
  Answered{at: Nat, yes: Bool}
  Unanswered{}

# What becomes of the step (crates/eukhe-types `ShellAnswer`): it runs, a
# client denied it, or no one answered
type Fate is Data:
  Runs{}
  Denied{}
  Unheard{}

# The sessions that showed the question (a frame to their clients), in
# order, and how it ended
type Out is Data:
  Out{shown: List<&2, Nat>, end: End}

def Out.end(o: Out) -> End:
  match o:
    case Out{s, e}:
      e

def Out.shown(o: Out) -> List<&2, Nat>:
  match o:
    case Out{s, e}:
      s

# The clients of the session answer
def Desk.says(d: Desk) -> Bool:
  match d:
    case Away{}:
      False{}
    case Left{}:
      False{}
    case Says{y}:
      True{}

# No session of the chain has clients that answer
def quiet(ds: List<&2, Desk>) -> Bool:
  match ds:
    case Nil{}:
      True{}
    case d <> rest:
      Bool.and(Bool.not(Desk.says(d)), quiet(rest))

def End.up(e: End) -> End:
  match e:
    case Answered{a, y}:
      Answered{1n+a, y}
    case Unanswered{}:
      Unanswered{}

def ups(xs: List<&2, Nat>) -> List<&2, Nat>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> rest:
      (1n+x) <> ups(rest)

# The parent's outcome, as its child sees it: every session one further
def Out.up(o: Out) -> Out:
  match o:
    case Out{s, e}:
      Out{ups(s), End.up(e)}

# The asker (session 0) showed the question before its parent's outcome
def Out.first(o: Out) -> Out:
  match o:
    case Out{s, e}:
      Out{0n <> s, e}

# Where the question of the first session of ds goes, and how it ends
def route(ds: List<&2, Desk>) -> Out:
  match ds:
    case Nil{}:
      Out{Nil{}, Unanswered{}}
    case d <> rest:
      match d:
        case Away{}:
          Out.up(route(rest))
        case Left{}:
          Out.first(Out.up(route(rest)))
        case Says{y}:
          Out{[0n], Answered{0n, y}}

# The grant of a session whose user sets none (`shell.autoEffects`): the
# reversible effects, read, write (inside the workspace) and exec
# (spec/README.md I1)
def default_auto() -> List<&2, Nat>:
  [0n, 1n, 2n]

def yes.fate(y: Bool) -> Fate:
  match y:
    case True{}:
      Runs{}
    case False{}:
      Denied{}

# What becomes of a step with the effects es whose question ended so: a
# yes runs it; a no, or no answer, refuses it
def fate(+es: List<&2, Nat>, e: End) -> Fate:
  match e:
    case Answered{a, y}:
      yes.fate(y)
    case Unanswered{}:
      Unheard{}

# How a step goes: within the auto set it runs, and no one is asked; else
# its question goes its route o, and the step fares f
type Gated is Data:
  Unasked{}
  Routed{o: Out, f: Fate}

def routed(+es: List<&2, Nat>, +o: Out) -> Gated:
  Routed{o, fate(es, Out.end(o))}

def gate.by(w: Bool, +es: List<&2, Nat>, ds: List<&2, Desk>) -> Gated:
  match w:
    case True{}:
      Unasked{}
    case False{}:
      routed(es, route(ds))

# A step with the effects es, under the auto set auto, of the session
# that heads the chain ds
def gate(+auto: List<&2, Nat>, +es: List<&2, Nat>, ds: List<&2, Desk>) -> Gated:
  gate.by(Tl.within(es, auto), es, ds)

# One session's decision (crates/eukhe-daemon/src/shell_approval.rs
# `route`, run as the compiled core): who answers a question of the
# session now
type Here is Data:
  Nobody{}       # no client is attached
  Clients{}      # a client is attached (the TUI, the dashboard, the phone gateway)

# What the session does with the question
type Next is Data:
  Show{}         # a frame to its clients: their answer, or their leaving, decides
  Relay{}        # hand it to the parent session (`relay_shell_approval`)
  Ends{f: Fate}  # it ends here: the step fares f

# a session with no client: up to its parent, else no one answers
def next.alone(+es: List<&2, Nat>, parent: Bool) -> Next:
  match parent:
    case True{}:
      Relay{}
    case False{}:
      Ends{fate(es, Out.end(route(Nil{})))}

# what a session that is h, with a parent or not, does with the question
# of a step with the effects es
def next(+es: List<&2, Nat>, h: Here, parent: Bool) -> Next:
  match h:
    case Nobody{}:
      next.alone(es, parent)
    case Clients{}:
      Show{}

# what the answer yes of a session's clients makes of the step
def answered(+es: List<&2, Nat>, yes: Bool) -> Fate:
  fate(es, Out.end(route([Says{yes}])))

# One session's desk
# ------------------
# The open questions of one session, as the compiled core keeps them (the
# stepper of bend/ffi_approvals.bend; crates/eukhe-daemon/src/
# shell_approval.rs): Rust sends each event as it arrives (a question
# reaches the session, a client answers, a client detaches, a step stops
# waiting, the parent session answers a relay) and does what the desk
# answers. The desk decides, with `next` and `answered` above, what each
# event means: which question an answer belongs to (the oldest open one
# whose frame names its request id and waits on the clients), whether it
# is still open (the first answer wins), and where a question goes when
# its clients leave.

# what an open question waits on
type Wait is Data:
  OnClients{}  # its frame is out: the session's clients answer, or leave
  OnParent{}   # it went up to the parent session

# an open question: the key the desk gave it, the request id its frame
# names, the step's effects, what it waits on
type Q is Data:
  Q{key: Nat, id: String, es: List<&2, Nat>, wait: Wait}

# the desk: the next key, the open questions oldest first
type D is Data:
  D{next: Nat, qs: List<&2, Q>}

def D.empty() -> D:
  D{0n, Nil{}}

type Ev is Data:
  EAsk{id: String, es: List<&2, Nat>, h: Here, parent: Bool}  # a question reaches the session (its own step's, or a child's relay)
  EAnswer{id: String, yes: Bool}                              # a client answers the question its frame named
  EDetach{h: Here, parent: Bool}                              # a client detached: who answers the session now
  ECancel{key: Nat}                                           # the step stopped waiting
  EBack{key: Nat, f: Fate}                                    # the parent's answer to a relay (Unheard when it failed)

type Act is Data:
  AShow{key: Nat}           # send the question's frame to the clients
  ASettle{key: Nat}         # tell the clients the question is settled
  ARelay{key: Nat}          # hand the question to the parent session
  AWake{key: Nat, f: Fate}  # the step fares f
  ATook{}                   # the client's answer was taken
  AGone{}                   # no open question takes it: refused

# a and b are the same text, char by char
def same(a: String, b: String) -> Bool:
  match a b:
    case SNil{} SNil{}:
      True{}
    case SNil{} SCon{y, v}:
      False{}
    case SCon{x, u} SNil{}:
      False{}
    case SCon{x, u} SCon{y, v}:
      Bool.and(Char.is_eq(x, y), same(u, v))

def Wait.clients(w: Wait) -> Bool:
  match w:
    case OnClients{}:
      True{}
    case OnParent{}:
      False{}

# a question taken out of a list, and the rest in order
type QF is Data:
  QFound{q: Q, rest: List<&2, Q>}
  QNone{}

def pick.q(hit: Bool, q: Q, +rest: List<&2, Q>, r: QF) -> QF:
  match hit:
    case True{}:
      QFound{q, rest}
    case False{}:
      match r:
        case QFound{q2, r2}:
          QFound{q2, q <> r2}
        case QNone{}:
          QNone{}

# the oldest question waiting on the clients whose frame names id
def find(qs: List<&2, Q>, +id: String) -> QF:
  match qs:
    case Nil{}:
      QNone{}
    case q <> +rest:
      match q:
        case Q{k, +i, es, +w}:
          pick.q(Bool.and(Wait.clients(w), same(i, id)), Q{k, i, es, w}, rest, find(rest, id))

# the question with key key
def take(qs: List<&2, Q>, +key: Nat) -> QF:
  match qs:
    case Nil{}:
      QNone{}
    case q <> +rest:
      match q:
        case Q{+k, i, es, w}:
          pick.q(Nat.is_eq(k, key), Q{k, i, es, w}, rest, take(rest, key))

def ask.with(+k: Nat, +id: String, +es: List<&2, Nat>, +qs: List<&2, Q>, x: Next) -> D & List<&2, Act>:
  match x:
    case Show{}:
      (D{1n+k, List.append(&2, Q, qs, [Q{k, id, es, OnClients{}}])}, [AShow{k}])
    case Relay{}:
      (D{1n+k, List.append(&2, Q, qs, [Q{k, id, es, OnParent{}}])}, [ARelay{k}])
    case Ends{f}:
      (D{1n+k, qs}, [AWake{k, f}])

def answer.with(+n: Nat, +qs: List<&2, Q>, +yes: Bool, r: QF) -> D & List<&2, Act>:
  match r:
    case QFound{q, rest}:
      match q:
        case Q{+k, i, +es, w}:
          (D{n, rest}, [ASettle{k}, AWake{k, answered(es, yes)}, ATook{}])
    case QNone{}:
      (D{n, qs}, [AGone{}])

def reroute.q(w: Wait, +k: Nat, +i: String, +es: List<&2, Nat>, x: Next, r: List<&2, Q> & List<&2, Act>) -> List<&2, Q> & List<&2, Act>:
  match w:
    case OnParent{}:
      (qs, as) = r
      (Q{k, i, es, OnParent{}} <> qs, as)
    case OnClients{}:
      match x:
        case Show{}:
          (qs, as) = r
          (Q{k, i, es, OnClients{}} <> qs, as)
        case Relay{}:
          (qs, as) = r
          (Q{k, i, es, OnParent{}} <> qs, ASettle{k} <> ARelay{k} <> as)
        case Ends{f}:
          (qs, as) = r
          (qs, ASettle{k} <> AWake{k, f} <> as)

# every question waiting on the clients goes where `next` sends it now
def reroute(qs: List<&2, Q>, +h: Here, +parent: Bool) -> List<&2, Q> & List<&2, Act>:
  match qs:
    case Nil{}:
      (Nil{}, Nil{})
    case q <> rest:
      match q:
        case Q{k, i, +es, w}:
          reroute.q(w, k, i, es, next(es, h, parent), reroute(rest, h, parent))

def detach.put(+n: Nat, r: List<&2, Q> & List<&2, Act>) -> D & List<&2, Act>:
  (rq, as) = r
  (D{n, rq}, as)

def detach.with(+n: Nat, +qs: List<&2, Q>, h: Here, +parent: Bool) -> D & List<&2, Act>:
  match h:
    case Clients{}:
      (D{n, qs}, Nil{})
    case Nobody{}:
      detach.put(n, reroute(qs, Nobody{}, parent))

def cancel.with(+n: Nat, +qs: List<&2, Q>, r: QF) -> D & List<&2, Act>:
  match r:
    case QFound{q, rest}:
      match q:
        case Q{+k, i, es, w}:
          match w:
            case OnClients{}:
              (D{n, rest}, [ASettle{k}])
            case OnParent{}:
              (D{n, rest}, Nil{})
    case QNone{}:
      (D{n, qs}, Nil{})

def back.with(+n: Nat, +qs: List<&2, Q>, +f: Fate, r: QF) -> D & List<&2, Act>:
  match r:
    case QFound{q, rest}:
      match q:
        case Q{+k, i, es, w}:
          match w:
            case OnParent{}:
              (D{n, rest}, [AWake{k, f}])
            case OnClients{}:
              (D{n, qs}, Nil{})
    case QNone{}:
      (D{n, qs}, Nil{})

# one event: the desk after it, and what Rust does (an ask's key is the
# desk's next key before it)
def dstep(d: D, e: Ev) -> D & List<&2, Act>:
  match d:
    case D{+n, +qs}:
      match e:
        case EAsk{id, +es, +h, +p}:
          ask.with(n, id, es, qs, next(es, h, p))
        case EAnswer{+id, yes}:
          answer.with(n, qs, yes, find(qs, id))
        case EDetach{h, p}:
          detach.with(n, qs, h, p)
        case ECancel{+key}:
          cancel.with(n, qs, take(qs, key))
        case EBack{+key, f}:
          back.with(n, qs, f, take(qs, key))
