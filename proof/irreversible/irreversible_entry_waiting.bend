# eukhe: the proof of LAW irreversible_entry_waiting

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

def Irreversible.irreversible_entry_waiting(es, fl):
  +rhs = Irreversible.ir_w_ns(S.waiting(es), "")
  %IRB.ie.nat(4n, Irreversible.ir_w_ledger(es, ""), {==}) : {FIr.ffi.got(FIr.get_req.go(_)) == rhs : String}
  %Equal.sym(Wr.Dec<List<&2, Ir.Entry>>, FIr.get_ledger(Irreversible.ir_w_ledger(es, "")), Wr.Got{es, ""}, IRB.ie.ledger(es, "", fl)) : {FIr.ffi.got(FIr.get_waiting(_)) == rhs : String}
  %Irreversible.irreversible_waiting_is_spec(es) : {FIr.ffi.got(FIr.get_waiting(Wr.Got{es, ""})) == Irreversible.ir_w_ns(_, "") : String}
  {==}
