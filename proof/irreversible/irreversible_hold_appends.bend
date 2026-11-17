# eukhe: the proof of LAW irreversible_hold_appends

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

def Irreversible.irreversible_hold_appends(es, i, new):
  %Equal.sym(Bool, Tl.elem(Ir.ids(es), i), False{}, Equal.trans(Bool, Tl.elem(Ir.ids(es), i), S.has(Irreversible.ir_ids(es), i), False{}, IRB.il.known(es, i), new)) : {S.waiting(Ir.hold.go(_, es, i)) == List.append(&2, Nat, S.waiting(es), [i]) : List<&2, Nat>}
  IRB.irb.waiting_snoc(es, i)
