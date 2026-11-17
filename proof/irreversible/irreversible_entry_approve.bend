# eukhe: the proof of LAW irreversible_entry_approve

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

def Irreversible.irreversible_entry_approve(w, es, fw, fl):
  +r1 = Irreversible.ir_w_which(w, Irreversible.ir_w_ledger(es, ""))
  +rhs = Irreversible.ir_w_ns(Ir.Approved.run(Ir.approve(w, es)), Irreversible.ir_w_ledger(Ir.Approved.ledger(Ir.approve(w, es)), ""))
  %IRB.ie.nat(3n, r1, {==}) : {FIr.ffi.got(FIr.get_req.go(_)) == rhs : String}
  %Equal.sym(Wr.Dec<Ir.Which>, FIr.get_which(r1), Wr.Got{w, Irreversible.ir_w_ledger(es, "")}, IRB.ie.which(w, Irreversible.ir_w_ledger(es, ""), fw)) : {FIr.ffi.got(FIr.get_approve(_)) == rhs : String}
  IRB.ie.approve_tail(w, es, fl)
