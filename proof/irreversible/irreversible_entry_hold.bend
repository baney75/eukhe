# eukhe: the proof of LAW irreversible_entry_hold

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

# The ledger
# ----------

def Irreversible.irreversible_entry_hold(es, i, fl, fi):
  +r1 = Irreversible.ir_w_ledger(es, Wr.put_nat(i, ""))
  +rhs = Irreversible.ir_w_ledger(Ir.hold(es, i), "")
  %IRB.ie.nat(2n, r1, {==}) : {FIr.ffi.got(FIr.get_req.go(_)) == rhs : String}
  %Equal.sym(Wr.Dec<List<&2, Ir.Entry>>, FIr.get_ledger(r1), Wr.Got{es, Wr.put_nat(i, "")}, IRB.ie.ledger(es, Wr.put_nat(i, ""), fl)) : {FIr.ffi.got(FIr.get_hold(_)) == rhs : String}
  %IRB.ie.nat(i, "", fi) : {FIr.ffi.got(FIr.get_hold.id(es, _)) == rhs : String}
  IRB.ie.put_ledger(Ir.hold(es, i), "")
