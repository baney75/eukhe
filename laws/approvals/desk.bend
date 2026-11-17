# eukhe: the laws of area approvals, section desk: one session's desk, the
# state machine of its open questions (bend/approvals.bend `dstep`), which
# the compiled core runs as the stepper of bend/ffi_approvals.bend
# (laws/approvals/entry.bend ties the entry to it)

import Base
import ../../bend/approvals.bend as Am
import ../base.bend as LawsBase

# LAWS (G5): Rust (crates/eukhe-daemon/src/shell_approval.rs) sends each
# event of a session as it arrives and does what the desk answers; the
# desk decides what the event means. Every input is free: any desk, any
# request ids, effects, sessions, answers, detaches, keys, fates, in any
# order.
#
# Requirements (sources: the module doc of crates/eukhe-daemon/src/
# shell_approval.rs and its tests, named per row; these rows are eukhe's
# own, stated by no outside source):
# D1: a question that reaches the session gets the desk's next key; the
#     session's decision sends it to its clients (shown; open, waiting on
#     the clients), to the parent session (relayed; open, waiting on the
#     parent), or ends it (woken with its fate; not kept). The decision:
#     any attached client shows it (the TUI, the dashboard, the phone
#     gateway, which asks over Signal); no client relays it to the
#     parent, or at a root ends it unanswered (approvals_desk_ask).
# D2: a client's answer goes to the oldest open question waiting on the
#     clients whose frame names its request id (char by char): that one is
#     settled, woken with the fate of the answer (yes runs the step, no
#     denies it), the answer taken, the question removed; with no such
#     question the answer is refused (gone) and the desk is unchanged: the
#     first answer wins (test each_answer_wakes_one_question_once;
#     approvals_desk_answer_takes_oldest, approvals_desk_answer_gone).
# D3: a detach that leaves no client sends every question waiting on the
#     clients where the session's decision now sends it: up to the parent
#     (settled, relayed) or ended (settled, woken), in order; a detach
#     that leaves a client changes nothing (test
#     a_question_its_clients_leave_goes_up_the_chain;
#     approvals_desk_detach_pieces, approvals_desk_detach_one).
# D4: a step that stops waiting removes its question (settled if it waited
#     on the clients); another key changes nothing (test
#     every_end_of_a_wait_settles_the_question_for_the_clients;
#     approvals_desk_cancel_removes, approvals_desk_unknown_key).
# D5: the parent's answer to a relayed question wakes it with that fate and
#     removes it; for any other key (one waiting on the clients, or none)
#     it changes nothing (approvals_desk_back_wakes,
#     approvals_desk_unknown_key).
# D6: a question waits on the clients only while a client is attached (no
#     lost wake-up); a root never relays (approvals_desk_never_stranded,
#     approvals_desk_root_never_relays).
# D7: the desks of a chain of sessions route a question exactly as
#     bend/approvals.bend `route` does, over the stepper
#     (approvals_desk_is_route).
# D8: over every sequence of events from the empty desk, no key is woken
#     twice: Rust wakes each waiting step at most once, whatever the order
#     of answers, detaches, cancels and the parent's answers
#     (approvals_desk_wakes_once).
# D9: a question no one answered is never a yes: a step runs only on a
#     client's answer yes or the parent's answer that runs it; a session
#     with no client, clients that all leave, a phone gateway that never
#     answers and a step that stops waiting all leave it not approved
#     (spec/README.md I5: silence is never a yes;
#     approvals_desk_runs_only_on_a_yes).

# Spec helpers
# ------------

# x is one of xs
def sp_has(xs: List<&2, Nat>, +x: Nat) -> Bool:
  match xs:
    case Nil{}:
      False{}
    case y <> rest:
      Bool.or(Nat.is_eq(y, x), sp_has(rest, x))

# D2: the clients' answer: yes runs the step, no denies it
def sp_answered(+es: List<&2, Nat>, yes: Bool) -> Am.Fate:
  match yes:
    case True{}:
      Am.Runs{}
    case False{}:
      Am.Denied{}

# a question waits on the session's clients (its frame is out)
def sp_clients(w: Am.Wait) -> Bool:
  match w:
    case Am.OnClients{}:
      True{}
    case Am.OnParent{}:
      False{}

# Terms of the step laws (D2, D4, D5)
# -----------------------------------

# the question q waits on the clients under the request id id (char by
# char)
def ds_open_for(q: Am.Q, +id: String) -> Bool:
  match q:
    case Am.Q{k, i, es, w}:
      Bool.and(sp_clients(w), LawsBase.text_same(i, id))

# no question of qs waits on the clients under the request id id
def ds_none_open(qs: List<&2, Am.Q>, +id: String) -> Bool:
  match qs:
    case Nil{}:
      True{}
    case q <> t:
      Bool.and(Bool.not(ds_open_for(q, id)), ds_none_open(t, id))

# no question of qs has the key k
def ds_no_key(qs: List<&2, Am.Q>, +k: Nat) -> Bool:
  match qs:
    case Nil{}:
      True{}
    case q <> t:
      match q:
        case Am.Q{j, i, es, w}:
          Bool.and(Bool.not(Nat.is_eq(j, k)), ds_no_key(t, k))

# Terms of the safety laws (D6)
# -----------------------------

def ds_desk(r: Am.D & List<&2, Am.Act>) -> Am.D:
  (d, as) = r
  d

def ds_acts(r: Am.D & List<&2, Am.Act>) -> List<&2, Am.Act>:
  (d, as) = r
  as

def ds_next(d: Am.D) -> Nat:
  match d:
    case Am.D{n, qs}:
      n

def ds_qs(d: Am.D) -> List<&2, Am.Q>:
  match d:
    case Am.D{n, qs}:
      qs

def ds_any_clients(qs: List<&2, Am.Q>) -> Bool:
  match qs:
    case Nil{}:
      False{}
    case q <> t:
      match q:
        case Am.Q{k, i, es, w}:
          Bool.or(sp_clients(w), ds_any_clients(t))

def ds_any_relay(as: List<&2, Am.Act>) -> Bool:
  match as:
    case Nil{}:
      False{}
    case a <> t:
      match a:
        case Am.ARelay{k}:
          True{}
        case Am.AShow{k}:
          ds_any_relay(t)
        case Am.ASettle{k}:
          ds_any_relay(t)
        case Am.AWake{k, f}:
          ds_any_relay(t)
        case Am.ATook{}:
          ds_any_relay(t)
        case Am.AGone{}:
          ds_any_relay(t)

# Terms of the wake-once law (D8)
# -------------------------------

def dw_hit(b: Bool, c: Nat) -> Nat:
  match b:
    case True{}:
      1n+c
    case False{}:
      c

# how many times k is in xs
def dw_count(xs: List<&2, Nat>, +k: Nat) -> Nat:
  match xs:
    case Nil{}:
      0n
    case x <> t:
      dw_hit(Nat.is_eq(x, k), dw_count(t, k))

# the keys the acts wake, in order
def dw_woke(as: List<&2, Am.Act>) -> List<&2, Nat>:
  match as:
    case Nil{}:
      Nil{}
    case a <> t:
      match a:
        case Am.AWake{k, f}:
          k <> dw_woke(t)
        case Am.AShow{k}:
          dw_woke(t)
        case Am.ASettle{k}:
          dw_woke(t)
        case Am.ARelay{k}:
          dw_woke(t)
        case Am.ATook{}:
          dw_woke(t)
        case Am.AGone{}:
          dw_woke(t)

# every key the desk's replies wake, fed the events es from the desk d, in
# order
def dw_run(es: List<&2, Am.Ev>, +d: Am.D) -> List<&2, Nat>:
  match es:
    case Nil{}:
      Nil{}
    case +e <> t:
      List.append(&2, Nat, dw_woke(ds_acts(Am.dstep(d, e))), dw_run(t, ds_desk(Am.dstep(d, e))))

# The chain (D7)
# --------------
# A question of the asking session, through the desks of its chain: each
# session's clients do what its Desk says (away: no client; left: shown,
# then every client detaches; says: shown, then the first answer). A relay
# asks the parent's desk, and its fate comes back to the session's desk.

# who answers the session when the question reaches it
def ch_here(d: Am.Desk) -> Am.Here:
  match d:
    case Am.Away{}:
      Am.Nobody{}
    case Am.Left{}:
      Am.Clients{}
    case Am.Says{y}:
      Am.Clients{}

# what the clients do once the question is shown
def ch_then(d: Am.Desk, +p: Bool) -> Am.Ev:
  match d:
    case Am.Away{}:
      Am.EDetach{Am.Nobody{}, p}
    case Am.Left{}:
      Am.EDetach{Am.Nobody{}, p}
    case Am.Says{y}:
      Am.EAnswer{"q", y}

# the fate the acts wake key 0 with, if any; whether they show or relay it
def ch_woke(as: List<&2, Am.Act>) -> Maybe<&2, Am.Fate>:
  match as:
    case Nil{}:
      None{}
    case a <> t:
      match a:
        case Am.AWake{k, f}:
          Some{f}
        case Am.AShow{k}:
          ch_woke(t)
        case Am.ASettle{k}:
          ch_woke(t)
        case Am.ARelay{k}:
          ch_woke(t)
        case Am.ATook{}:
          ch_woke(t)
        case Am.AGone{}:
          ch_woke(t)

def ch_shows(as: List<&2, Am.Act>) -> Bool:
  match as:
    case Nil{}:
      False{}
    case a <> t:
      match a:
        case Am.AShow{k}:
          True{}
        case Am.ASettle{k}:
          ch_shows(t)
        case Am.ARelay{k}:
          ch_shows(t)
        case Am.AWake{k, f}:
          ch_shows(t)
        case Am.ATook{}:
          ch_shows(t)
        case Am.AGone{}:
          ch_shows(t)

def ch_up(xs: List<&2, Nat>) -> List<&2, Nat>:
  match xs:
    case Nil{}:
      Nil{}
    case x <> t:
      (1n+x) <> ch_up(t)

def ch_fate(m: Maybe<&2, Am.Fate>) -> Am.Fate:
  match m:
    case None{}:
      Am.Unheard{}
    case Some{f}:
      f

# the sessions that showed the question (0: this one) and its fate, from
# the parent's (shown, fate), after this session's desk relayed it
def ch_relayed(+shown0: List<&2, Nat>, r: List<&2, Nat> & Am.Fate, +d1: Am.D) -> List<&2, Nat> & Am.Fate:
  (up, f) = r
  (List.append(&2, Nat, shown0, ch_up(up)), ch_fate(ch_woke(ds_acts(Am.dstep(d1, Am.EBack{0n, f})))))

# what a reply did with the question: woke it (with its fate), relayed
# it, showed it, or nothing of these
type Ck is Data:
  CkWoke{f: Am.Fate}
  CkRelay{}
  CkShow{}
  CkNone{}

def ch_kind(as: List<&2, Am.Act>) -> Ck:
  match as:
    case Nil{}:
      CkNone{}
    case a <> t:
      match a:
        case Am.AWake{k, f}:
          CkWoke{f}
        case Am.ARelay{k}:
          CkRelay{}
        case Am.AShow{k}:
          CkShow{}
        case Am.ASettle{k}:
          ch_kind(t)
        case Am.ATook{}:
          ch_kind(t)
        case Am.AGone{}:
          ch_kind(t)

# after the clients of a session that showed the question acted (k), the
# parent's (shown, fate) up, the desk d2 after the clients' event
def ch_after(k: Ck, up: List<&2, Nat> & Am.Fate, +d2: Am.D) -> List<&2, Nat> & Am.Fate:
  match k:
    case CkWoke{f}:
      ([0n], f)
    case CkRelay{}:
      ch_relayed([0n], up, d2)
    case CkShow{}:
      ([0n], Am.Unheard{})
    case CkNone{}:
      ([0n], Am.Unheard{})

def ch_at.k(k: Ck, +d: Am.Desk, +p: Bool, +d1: Am.D, up: List<&2, Nat> & Am.Fate) -> List<&2, Nat> & Am.Fate:
  match k:
    case CkWoke{f}:
      (Nil{}, f)
    case CkRelay{}:
      ch_relayed(Nil{}, up, d1)
    case CkShow{}:
      ch_after(ch_kind(ds_acts(Am.dstep(d1, ch_then(d, p)))), up, ds_desk(Am.dstep(d1, ch_then(d, p))))
    case CkNone{}:
      (Nil{}, Am.Unheard{})

# the session with Desk d (a parent when p): its desk's step on the
# question (r1), and the parent's (shown, fate) up
def ch_at(+d: Am.Desk, +p: Bool, r1: Am.D & List<&2, Am.Act>, up: List<&2, Nat> & Am.Fate) -> List<&2, Nat> & Am.Fate:
  (d1, as1) = r1
  ch_at.k(ch_kind(as1), d, p, d1, up)

# the question of the step with the effects es at the first session of
# ds, through the desks of the chain: the sessions that showed it, and its
# fate
def ch_run(ds: List<&2, Am.Desk>, +es: List<&2, Nat>) -> List<&2, Nat> & Am.Fate:
  match ds:
    case Nil{}:
      (Nil{}, Am.Unheard{})
    case +d <> +rest:
      ch_at(d, Bool.not(List.is_empty(&2, Am.Desk, rest)), Am.dstep(Am.D{0n, Nil{}}, Am.EAsk{"q", es, ch_here(d), Bool.not(List.is_empty(&2, Am.Desk, rest))}), ch_run(rest, es))

# Laws
# ----

# LAW approvals_desk_ask (D1): from every desk (next key n, open questions
# qs) and every question (request id, effects, parent or not), the
# question gets the key n and the next key becomes n + 1: a session with
# an interactive client shows it (kept last, waiting on the clients); a
# session with no client and a parent relays it (kept last, waiting on the
# parent); a root with no client wakes it at once, unanswered (not kept);
# qs stay as they were, in order
# EXCLUDES: a key reused or skipped, a new question put before older ones,
#   a frame on a session with no client, a relay from a session with a
#   client or from a root, an unanswered question kept open, an older
#   question dropped or moved
law approvals_desk_ask:
  for +n: Nat
  for +qs: List<&2, Am.Q>
  for +id: String
  for +es: List<&2, Nat>
  for +p: Bool
  {Am.dstep(Am.D{n, qs}, Am.EAsk{id, es, Am.Clients{}, p}) == (Am.D{1n+n, List.append(&2, Am.Q, qs, [Am.Q{n, id, es, Am.OnClients{}}])}, [Am.AShow{n}]) : Am.D & List<&2, Am.Act>} & {Am.dstep(Am.D{n, qs}, Am.EAsk{id, es, Am.Nobody{}, True{}}) == (Am.D{1n+n, List.append(&2, Am.Q, qs, [Am.Q{n, id, es, Am.OnParent{}}])}, [Am.ARelay{n}]) : Am.D & List<&2, Am.Act>} & {Am.dstep(Am.D{n, qs}, Am.EAsk{id, es, Am.Nobody{}, False{}}) == (Am.D{1n+n, qs}, [Am.AWake{n, Am.Unheard{}}]) : Am.D & List<&2, Am.Act>}

# LAW approvals_desk_answer_takes_oldest (D2): a client's answer yes to
# the request id id, on a desk whose questions are pre, then q (key k,
# waiting on the clients under id), then post, where no question of pre
# waits on the clients under id (q is the oldest that does): q is settled,
# woken with the answer's fate (yes runs the step, no denies it, whatever
# its effects), the answer taken, and q leaves the desk; pre, post and the
# next key stay
# EXCLUDES: an answer given to a newer question than the oldest its frame
#   names, or to a relayed one; the wrong question settled or woken; a yes
#   read as a no; a question kept after its answer; other questions lost
#   or reordered
law approvals_desk_answer_takes_oldest:
  for +n: Nat
  for +pre: List<&2, Am.Q>
  for +k: Nat
  for +id: String
  for +es: List<&2, Nat>
  for +post: List<&2, Am.Q>
  for +yes: Bool
  for none: {ds_none_open(pre, id) == True{} : Bool}
  {Am.dstep(Am.D{n, List.append(&2, Am.Q, pre, Am.Q{k, id, es, Am.OnClients{}} <> post)}, Am.EAnswer{id, yes}) == (Am.D{n, List.append(&2, Am.Q, pre, post)}, [Am.ASettle{k}, Am.AWake{k, sp_answered(es, yes)}, Am.ATook{}]) : Am.D & List<&2, Am.Act>}

# LAW approvals_desk_answer_gone (D2): a client's answer to the request id
# id, on a desk where no question waits on the clients under id (none was
# asked, or each was answered, relayed or ended), is refused (gone):
# nothing is settled or woken, and the desk stays as it was. With
# approvals_desk_answer_takes_oldest: the first answer wins
# EXCLUDES: a second answer taken, an answer that wakes a relayed question
#   or one of another id, a refusal that changes the desk
law approvals_desk_answer_gone:
  for +n: Nat
  for +qs: List<&2, Am.Q>
  for +id: String
  for +yes: Bool
  for none: {ds_none_open(qs, id) == True{} : Bool}
  {Am.dstep(Am.D{n, qs}, Am.EAnswer{id, yes}) == (Am.D{n, qs}, [Am.AGone{}]) : Am.D & List<&2, Am.Act>}

# LAW approvals_desk_detach_pieces (D3): a detach treats each open question
# on its own, in order: on a desk whose questions are xs then ys (any
# session h left, parent or not), the questions it keeps are those it
# keeps of xs alone, then those of ys alone, and its acts are those for xs
# alone, then those for ys alone; the next key stays
# EXCLUDES: a detach that drops, duplicates or reorders questions or acts,
#   or treats a question by its neighbours (only the first, or the last,
#   rerouted)
law approvals_desk_detach_pieces:
  for +n: Nat
  for +xs: List<&2, Am.Q>
  for +ys: List<&2, Am.Q>
  for +h: Am.Here
  for +p: Bool
  {Am.dstep(Am.D{n, List.append(&2, Am.Q, xs, ys)}, Am.EDetach{h, p}) == (Am.D{n, List.append(&2, Am.Q, ds_qs(ds_desk(Am.dstep(Am.D{n, xs}, Am.EDetach{h, p}))), ds_qs(ds_desk(Am.dstep(Am.D{n, ys}, Am.EDetach{h, p}))))}, List.append(&2, Am.Act, ds_acts(Am.dstep(Am.D{n, xs}, Am.EDetach{h, p})), ds_acts(Am.dstep(Am.D{n, ys}, Am.EDetach{h, p})))) : Am.D & List<&2, Am.Act>}

# LAW approvals_desk_detach_one (D3, D6): a detach on a desk of one open
# question (key k, request id i, effects es): one waiting on the parent
# stays as it is, whoever is left; one waiting on the clients stays while
# a client is attached; when none is, it is settled for the clients and
# then goes exactly where the same question (key k, id i, effects es)
# asked of the session now goes, as the step of the empty desk with next
# key k on that ask says: up to the parent (kept, waiting on the parent)
# or ended (woken); the next key stays
# EXCLUDES: a relayed question moved or woken by a detach, a question its
#   remaining clients still see moved, a question left waiting on clients
#   that all left, a question sent up from a root, a left question not
#   settled for the clients
law approvals_desk_detach_one:
  for +n: Nat
  for +k: Nat
  for +i: String
  for +es: List<&2, Nat>
  for +h: Am.Here
  for +p: Bool
  {Am.dstep(Am.D{n, [Am.Q{k, i, es, Am.OnParent{}}]}, Am.EDetach{h, p}) == (Am.D{n, [Am.Q{k, i, es, Am.OnParent{}}]}, Nil{}) : Am.D & List<&2, Am.Act>} & {Am.dstep(Am.D{n, [Am.Q{k, i, es, Am.OnClients{}}]}, Am.EDetach{Am.Clients{}, p}) == (Am.D{n, [Am.Q{k, i, es, Am.OnClients{}}]}, Nil{}) : Am.D & List<&2, Am.Act>} & {Am.dstep(Am.D{n, [Am.Q{k, i, es, Am.OnClients{}}]}, Am.EDetach{Am.Nobody{}, p}) == (Am.D{n, ds_qs(ds_desk(Am.dstep(Am.D{k, Nil{}}, Am.EAsk{i, es, Am.Nobody{}, p})))}, Am.ASettle{k} <> ds_acts(Am.dstep(Am.D{k, Nil{}}, Am.EAsk{i, es, Am.Nobody{}, p}))) : Am.D & List<&2, Am.Act>}

# LAW approvals_desk_cancel_removes (D4): a step that stops waiting (key
# k) removes its question, the first with key k (no question of pre has
# k): settled for the clients when it waited on them, silently when it
# waited on the parent (the relay is dropped); nothing is woken; pre,
# post and the next key stay
# EXCLUDES: a cancelled question kept (an answer to it later taken), a
#   cancelled step woken, a frame left on the clients, another key's
#   question removed
law approvals_desk_cancel_removes:
  for +n: Nat
  for +pre: List<&2, Am.Q>
  for +k: Nat
  for +i: String
  for +es: List<&2, Nat>
  for +post: List<&2, Am.Q>
  for none: {ds_no_key(pre, k) == True{} : Bool}
  {Am.dstep(Am.D{n, List.append(&2, Am.Q, pre, Am.Q{k, i, es, Am.OnClients{}} <> post)}, Am.ECancel{k}) == (Am.D{n, List.append(&2, Am.Q, pre, post)}, [Am.ASettle{k}]) : Am.D & List<&2, Am.Act>} & {Am.dstep(Am.D{n, List.append(&2, Am.Q, pre, Am.Q{k, i, es, Am.OnParent{}} <> post)}, Am.ECancel{k}) == (Am.D{n, List.append(&2, Am.Q, pre, post)}, Nil{}) : Am.D & List<&2, Am.Act>}

# LAW approvals_desk_back_wakes (D5): the parent's answer (fate f) to key
# k, where the first question with key k (none of pre has k) waits on the
# parent: it is woken with f and leaves the desk; where that question
# waits on the clients (it was never relayed): nothing changes; pre, post
# and the next key stay
# EXCLUDES: a relayed question woken with another fate or kept after its
#   wake, a question its clients still see woken by a parent's answer
law approvals_desk_back_wakes:
  for +n: Nat
  for +pre: List<&2, Am.Q>
  for +k: Nat
  for +i: String
  for +es: List<&2, Nat>
  for +post: List<&2, Am.Q>
  for +f: Am.Fate
  for none: {ds_no_key(pre, k) == True{} : Bool}
  {Am.dstep(Am.D{n, List.append(&2, Am.Q, pre, Am.Q{k, i, es, Am.OnParent{}} <> post)}, Am.EBack{k, f}) == (Am.D{n, List.append(&2, Am.Q, pre, post)}, [Am.AWake{k, f}]) : Am.D & List<&2, Am.Act>} & {Am.dstep(Am.D{n, List.append(&2, Am.Q, pre, Am.Q{k, i, es, Am.OnClients{}} <> post)}, Am.EBack{k, f}) == (Am.D{n, List.append(&2, Am.Q, pre, Am.Q{k, i, es, Am.OnClients{}} <> post)}, Nil{}) : Am.D & List<&2, Am.Act>}

# LAW approvals_desk_unknown_key (D4, D5): a step that stops waiting, or a
# parent's answer (any fate f), for a key k no open question has changes
# nothing: no act, the desk kept
# EXCLUDES: a cancel or a parent's answer for one key that removes, settles
#   or wakes another key's question
law approvals_desk_unknown_key:
  for +n: Nat
  for +qs: List<&2, Am.Q>
  for +k: Nat
  for +f: Am.Fate
  for none: {ds_no_key(qs, k) == True{} : Bool}
  {Am.dstep(Am.D{n, qs}, Am.ECancel{k}) == (Am.D{n, qs}, Nil{}) : Am.D & List<&2, Am.Act>} & {Am.dstep(Am.D{n, qs}, Am.EBack{k, f}) == (Am.D{n, qs}, Nil{}) : Am.D & List<&2, Am.Act>}

# LAW approvals_desk_never_stranded (D6): from every desk, a detach that
# leaves no client leaves no question waiting on the clients
# EXCLUDES: a lost wake-up: a step that waits on clients that all left
law approvals_desk_never_stranded:
  for +d: Am.D
  for +p: Bool
  {ds_any_clients(ds_qs(ds_desk(Am.dstep(d, Am.EDetach{Am.Nobody{}, p})))) == False{} : Bool}

# LAW approvals_desk_root_never_relays (D6): from every desk, a root
# session (no parent) never relays, on a question or on a detach
# EXCLUDES: a relay to a parent that does not exist (a question lost)
law approvals_desk_root_never_relays:
  for +d: Am.D
  for +id: String
  for +es: List<&2, Nat>
  for +h: Am.Here
  {[ds_any_relay(ds_acts(Am.dstep(d, Am.EAsk{id, es, h, False{}}))), ds_any_relay(ds_acts(Am.dstep(d, Am.EDetach{h, False{}})))] == [False{}, False{}] : List<&2, Bool>}

# LAW approvals_desk_is_route (D7): the desks of a chain of sessions route
# a question exactly as `route` does: the sessions that show it, in order,
# and the fate of the step
# EXCLUDES: a relay past clients that answer, a frame on a session that
#   should not show it, a fate other than the route's
law approvals_desk_is_route:
  for +ds: List<&2, Am.Desk>
  for +es: List<&2, Nat>
  {ch_run(ds, es) == (Am.Out.shown(Am.route(ds)), Am.fate(es, Am.Out.end(Am.route(ds)))) : List<&2, Nat> & Am.Fate}

# LAW approvals_desk_example (D1-D6): anchor: a question shown and answered
# yes, a second answer refused, a relayed one woken by its parent's denial,
# a shown one that its clients leave at a root (woken unanswered)
law approvals_desk_example:
  {ds_acts(Am.dstep(Am.D{2n, [Am.Q{0n, "a", [1n], Am.OnClients{}}, Am.Q{1n, "b", [1n], Am.OnParent{}}]}, Am.EAnswer{"a", True{}})) == [Am.ASettle{0n}, Am.AWake{0n, Am.Runs{}}, Am.ATook{}] : List<&2, Am.Act>} & {ds_acts(Am.dstep(Am.D{2n, [Am.Q{1n, "b", [1n], Am.OnParent{}}]}, Am.EAnswer{"a", True{}})) == [Am.AGone{}] : List<&2, Am.Act>} & {ds_acts(Am.dstep(Am.D{2n, [Am.Q{1n, "b", [1n], Am.OnParent{}}]}, Am.EBack{1n, Am.Denied{}})) == [Am.AWake{1n, Am.Denied{}}] : List<&2, Am.Act>} & {ds_acts(Am.dstep(Am.D{1n, [Am.Q{0n, "a", [1n], Am.OnClients{}}]}, Am.EDetach{Am.Nobody{}, False{}})) == [Am.ASettle{0n}, Am.AWake{0n, Am.Unheard{}}] : List<&2, Am.Act>}

# LAW approvals_desk_wakes_once (D8): over every sequence of events, fed
# from the empty desk as Rust opens it, every key is woken at most once
# EXCLUDES: a step woken twice (a second answer, a detach or a parent's
#   answer that wakes a question already woken), a key given to two
#   questions, an ended question kept open
law approvals_desk_wakes_once:
  for +es: List<&2, Am.Ev>
  for +k: Nat
  {Nat.is_le(dw_count(dw_run(es, Am.D{0n, Nil{}}), k), 1n) == True{} : Bool}

# Terms of the yes law (D9)
# -------------------------

# the step runs
def dy_runs(f: Am.Fate) -> Bool:
  match f:
    case Am.Runs{}:
      True{}
    case Am.Denied{}:
      False{}
    case Am.Unheard{}:
      False{}

# the event e is a yes for the question with the key k: a client's answer
# yes, or the parent's answer to k that runs the step
def dy_said_yes(e: Am.Ev, +k: Nat) -> Bool:
  match e:
    case Am.EAsk{id, es, h, p}:
      False{}
    case Am.EAnswer{id, y}:
      y
    case Am.EDetach{h, p}:
      False{}
    case Am.ECancel{j}:
      False{}
    case Am.EBack{j, f}:
      Bool.and(Nat.is_eq(k, j), dy_runs(f))

# every wake among the acts that runs a step answers a yes of the event e
# for that step
def dy_yes_only(+e: Am.Ev, as: List<&2, Am.Act>) -> Bool:
  match as:
    case Nil{}:
      True{}
    case a <> t:
      match a:
        case Am.AWake{k, f}:
          Bool.and(Bool.or(Bool.not(dy_runs(f)), dy_said_yes(e, k)), dy_yes_only(e, t))
        case Am.AShow{k}:
          dy_yes_only(e, t)
        case Am.ASettle{k}:
          dy_yes_only(e, t)
        case Am.ARelay{k}:
          dy_yes_only(e, t)
        case Am.ATook{}:
          dy_yes_only(e, t)
        case Am.AGone{}:
          dy_yes_only(e, t)

# LAW approvals_desk_runs_only_on_a_yes (D9): from every desk, every event
# wakes a step to run only when the event is a yes for that step: a
# client's answer yes, or the parent's answer to that key that runs it. A
# question no one answered (a session with no client, clients that all
# left, the phone gateway asked and silent, a step that stopped waiting)
# never runs its step
# EXCLUDES: a session that runs a step no one approved (a rule that says
#   yes for an absent user, a detach or a cancel read as a yes, a denial
#   or an unanswered relay that runs, a parent's yes to another key)
law approvals_desk_runs_only_on_a_yes:
  for +d: Am.D
  for +e: Am.Ev
  {dy_yes_only(e, ds_acts(Am.dstep(d, e))) == True{} : Bool}

# LAW approvals_desk_fates_example (D1-D3): anchor: the fates the desk
# wakes steps with: a protected-path write (effects 1, 5) and a secret
# read (0, 6) reach a session with a client (the phone gateway counts as
# one) and are shown, never ruled; a write at a root with no client is
# unanswered; a protected-path write its clients approve runs; a secret
# read they deny is denied; a protected-path write the clients leave goes
# up to the parent, and one they leave at a root is unanswered
# EXCLUDES: a session that rules a step instead of asking, an approval
#   read as a no, a denial read as a yes, a left question that runs
law approvals_desk_fates_example:
  {ds_acts(Am.dstep(Am.D{0n, Nil{}}, Am.EAsk{"w", [1n, 5n], Am.Clients{}, True{}})) == [Am.AShow{0n}] : List<&2, Am.Act>} &
  {ds_acts(Am.dstep(Am.D{0n, Nil{}}, Am.EAsk{"s", [0n, 6n], Am.Clients{}, False{}})) == [Am.AShow{0n}] : List<&2, Am.Act>} &
  {ds_acts(Am.dstep(Am.D{3n, Nil{}}, Am.EAsk{"x", [1n], Am.Nobody{}, False{}})) == [Am.AWake{3n, Am.Unheard{}}] : List<&2, Am.Act>} &
  {ds_acts(Am.dstep(Am.D{1n, [Am.Q{0n, "w", [1n, 5n], Am.OnClients{}}]}, Am.EAnswer{"w", True{}})) == [Am.ASettle{0n}, Am.AWake{0n, Am.Runs{}}, Am.ATook{}] : List<&2, Am.Act>} &
  {ds_acts(Am.dstep(Am.D{1n, [Am.Q{0n, "s", [0n, 6n], Am.OnClients{}}]}, Am.EAnswer{"s", False{}})) == [Am.ASettle{0n}, Am.AWake{0n, Am.Denied{}}, Am.ATook{}] : List<&2, Am.Act>} &
  {ds_acts(Am.dstep(Am.D{1n, [Am.Q{0n, "w", [1n, 5n], Am.OnClients{}}]}, Am.EDetach{Am.Nobody{}, True{}})) == [Am.ASettle{0n}, Am.ARelay{0n}] : List<&2, Am.Act>} &
  {ds_acts(Am.dstep(Am.D{1n, [Am.Q{0n, "w", [1n, 5n], Am.OnClients{}}]}, Am.EDetach{Am.Nobody{}, False{}})) == [Am.ASettle{0n}, Am.AWake{0n, Am.Unheard{}}] : List<&2, Am.Act>}

# LAW approvals_desk_terms_exact: the terms of the desk laws mean what they
# say: a question waiting on the clients is seen, one on the parent is not;
# a relay among acts is seen, other acts are not; the woken keys are the
# wakes' keys in order; a count counts; a run collects every reply's wakes;
# a question waiting on the clients under an id is seen (not one on the
# parent, not one under another id), and so is a key; a wake that runs a
# step passes the yes check only after a yes for its key (a client's yes,
# the parent's runs for that key), and a wake that refuses always does
# EXCLUDES: a vacuous term (one constant answer) that would make
#   approvals_desk_never_stranded, approvals_desk_root_never_relays,
#   approvals_desk_wakes_once, approvals_desk_runs_only_on_a_yes, or the
#   premises of approvals_desk_answer_takes_oldest,
#   approvals_desk_answer_gone, approvals_desk_cancel_removes,
#   approvals_desk_back_wakes and approvals_desk_unknown_key say nothing
law approvals_desk_terms_exact:
  {[ds_any_clients([Am.Q{0n, "a", Nil{}, Am.OnParent{}}, Am.Q{1n, "b", Nil{}, Am.OnClients{}}]), ds_any_clients([Am.Q{0n, "a", Nil{}, Am.OnParent{}}]), ds_any_relay([Am.ASettle{0n}, Am.ARelay{0n}]), ds_any_relay([Am.AShow{0n}, Am.ASettle{0n}, Am.AWake{0n, Am.Runs{}}, Am.ATook{}, Am.AGone{}])] == [True{}, False{}, True{}, False{}] : List<&2, Bool>} &
  {dw_woke([Am.AShow{0n}, Am.AWake{1n, Am.Runs{}}, Am.ASettle{2n}, Am.ARelay{4n}, Am.AWake{3n, Am.Denied{}}, Am.ATook{}, Am.AGone{}]) == [1n, 3n] : List<&2, Nat>} &
  {[dw_count([1n, 3n, 1n], 1n), dw_count([1n, 3n, 1n], 2n)] == [2n, 0n] : List<&2, Nat>} &
  {dw_run([Am.EAsk{"a", [1n], Am.Nobody{}, False{}}, Am.EAsk{"b", [1n], Am.Clients{}, False{}}, Am.EAnswer{"b", False{}}], Am.D{0n, Nil{}}) == [0n, 1n] : List<&2, Nat>} &
  {[ds_none_open([Am.Q{0n, "a", Nil{}, Am.OnParent{}}, Am.Q{1n, "b", Nil{}, Am.OnClients{}}], "a"), ds_none_open([Am.Q{0n, "a", Nil{}, Am.OnParent{}}, Am.Q{1n, "b", Nil{}, Am.OnClients{}}], "b"), ds_no_key([Am.Q{0n, "a", Nil{}, Am.OnParent{}}], 1n), ds_no_key([Am.Q{2n, "a", Nil{}, Am.OnParent{}}, Am.Q{0n, "b", Nil{}, Am.OnClients{}}], 0n)] == [True{}, False{}, True{}, False{}] : List<&2, Bool>} &
  {[dy_yes_only(Am.EAsk{"a", [1n], Am.Clients{}, False{}}, [Am.AShow{0n}, Am.AWake{0n, Am.Runs{}}]), dy_yes_only(Am.EAnswer{"a", True{}}, [Am.ASettle{0n}, Am.AWake{0n, Am.Runs{}}, Am.ATook{}]), dy_yes_only(Am.EAnswer{"a", False{}}, [Am.AWake{0n, Am.Runs{}}]), dy_yes_only(Am.EBack{1n, Am.Runs{}}, [Am.AWake{0n, Am.Runs{}}]), dy_yes_only(Am.EBack{0n, Am.Runs{}}, [Am.AWake{0n, Am.Runs{}}]), dy_yes_only(Am.EBack{0n, Am.Denied{}}, [Am.AWake{0n, Am.Runs{}}]), dy_yes_only(Am.EDetach{Am.Nobody{}, False{}}, [Am.ASettle{0n}, Am.ARelay{1n}, Am.AWake{0n, Am.Unheard{}}, Am.AWake{2n, Am.Denied{}}, Am.AGone{}]), dy_yes_only(Am.ECancel{0n}, [Am.AWake{0n, Am.Unheard{}}, Am.AWake{0n, Am.Runs{}}])] == [False{}, True{}, False{}, False{}, True{}, False{}, True{}, False{}] : List<&2, Bool>}
