# eukhe: the proof of LAW irreversible_hold_known_keeps

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

def Irreversible.irreversible_hold_known_keeps(es, i, known):
  %Equal.sym(Bool, Tl.elem(Ir.ids(es), i), True{}, Equal.trans(Bool, Tl.elem(Ir.ids(es), i), S.has(Irreversible.ir_ids(es), i), True{}, IRB.il.known(es, i), known)) : {Ir.hold.go(_, es, i) == es : List<&2, Ir.Entry>}
  {==}
