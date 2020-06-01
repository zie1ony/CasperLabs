import * as CL from "../../../../contract-as/assembly";
import {Error, ErrorCode} from "../../../../contract-as/assembly/error";
import {U512} from "../../../../contract-as/assembly/bignum";
import {CLValue} from "../../../../contract-as/assembly/clvalue";
import {Key} from "../../../../contract-as/assembly/key";
import {getMainPurse} from "../../../../contract-as/assembly/account";
import {createPurse, transferFromPurseToPurse} from "../../../../contract-as/assembly/purse";
import {RuntimeArgs} from "../../../../contract-as/assembly/runtime_args";

const POS_ACTION = "bond";
const ARG_AMOUNT = "amount";
const ARG_PURSE = "purse";

export function call(): void {
    let proofOfStake = CL.getSystemContract(CL.SystemContract.ProofOfStake);
    let mainPurse = getMainPurse();
    let bondingPurse = createPurse();

    let amountBytes = CL.getNamedArg(ARG_AMOUNT);
    if (amountBytes === null) {
        Error.fromErrorCode(ErrorCode.MissingArgument).revert();
        return;
    }

    let amountResult = U512.fromBytes(amountBytes);
    if (amountResult.hasError()) {
        Error.fromErrorCode(ErrorCode.InvalidArgument).revert();
        return;
    }

    let amount = amountResult.value;

    let ret = transferFromPurseToPurse(
        mainPurse,
        bondingPurse,
        amount,
    );
    if (ret > 0) {
        Error.fromErrorCode(ErrorCode.Transfer).revert();
        return;
    }

    let bondingPurseValue = CLValue.fromURef(bondingPurse);
    let args = new Map<String, CLValue>();
    args.set(ARG_AMOUNT, CLValue.fromU512(amount));
    args.set(ARG_PURSE, bondingPurseValue);
    CL.callContract(proofOfStake, POS_ACTION, RuntimeArgs.fromMap(args));
}
