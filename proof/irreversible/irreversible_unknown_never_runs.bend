# eukhe: the proof of LAW irreversible_unknown_never_runs

import Base
import ../../bend/irreversible.bend as Ir
import ../../bend/ffi_irreversible.bend as FIr
import ../../bend/tools.bend as Tl
import ../../bend/approvals.bend as Am
import ../../bend/wire.bend as Wr
import ../../spec/irreversible/effects.bend as S
import ../../laws/irreversible.bend as Irreversible
import ../../laws/tools.bend as Tools
import ../../laws/wire.bend as Wire
import ../../laws/base.bend as LawsBase
import ./base.bend as IRB
import ../tools/auto_needs_every_effect.bend as PAutoNeedsEveryEffect

def Irreversible.irreversible_unknown_never_runs(effs, auto, m, c, has, u):
  +notin = Tools.auto_needs_every_effect(Tl.Step{0n, effs}, auto, c, Equal.trans(Bool, Tl.elem(effs, c), S.has(effs, c), True{}, IRB.irb.elem_has(effs, c), has), IRB.ir.unknown_out(c, auto, u))
  Equal.trans(Bool, Irreversible.ir_runs(Ir.route(effs, auto, m)), Tl.within(effs, auto), False{}, IRB.irb.runs_of(Tl.within(effs, auto), m), notin)
