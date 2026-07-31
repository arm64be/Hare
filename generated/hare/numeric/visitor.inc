    case JSC::op_to_primitive: {
        auto decoded = instruction->as<JSC::OpToPrimitive>();
        if (!emitSignedOperand(visitorContext, "operand.op_to_primitive.dst", 27, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_to_primitive.src", 27, 0, static_cast<int64_t>(decoded.m_src.offset())))
            return false;
        return true;
    }
    case JSC::op_eq: {
        auto decoded = instruction->as<JSC::OpEq>();
        if (!emitSignedOperand(visitorContext, "operand.op_eq.dst", 17, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_eq.lhs", 17, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_eq.rhs", 17, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        return true;
    }
    case JSC::op_neq: {
        auto decoded = instruction->as<JSC::OpNeq>();
        if (!emitSignedOperand(visitorContext, "operand.op_neq.dst", 18, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_neq.lhs", 18, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_neq.rhs", 18, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        return true;
    }
    case JSC::op_stricteq: {
        auto decoded = instruction->as<JSC::OpStricteq>();
        if (!emitSignedOperand(visitorContext, "operand.op_stricteq.dst", 23, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_stricteq.lhs", 23, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_stricteq.rhs", 23, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        return true;
    }
    case JSC::op_nstricteq: {
        auto decoded = instruction->as<JSC::OpNstricteq>();
        if (!emitSignedOperand(visitorContext, "operand.op_nstricteq.dst", 24, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_nstricteq.lhs", 24, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_nstricteq.rhs", 24, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        return true;
    }
    case JSC::op_less: {
        auto decoded = instruction->as<JSC::OpLess>();
        if (!emitSignedOperand(visitorContext, "operand.op_less.dst", 19, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_less.lhs", 19, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_less.rhs", 19, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        return true;
    }
    case JSC::op_lesseq: {
        auto decoded = instruction->as<JSC::OpLesseq>();
        if (!emitSignedOperand(visitorContext, "operand.op_lesseq.dst", 21, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_lesseq.lhs", 21, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_lesseq.rhs", 21, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        return true;
    }
    case JSC::op_greater: {
        auto decoded = instruction->as<JSC::OpGreater>();
        if (!emitSignedOperand(visitorContext, "operand.op_greater.dst", 22, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_greater.lhs", 22, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_greater.rhs", 22, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        return true;
    }
    case JSC::op_greatereq: {
        auto decoded = instruction->as<JSC::OpGreatereq>();
        if (!emitSignedOperand(visitorContext, "operand.op_greatereq.dst", 24, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_greatereq.lhs", 24, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_greatereq.rhs", 24, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        return true;
    }
    case JSC::op_below: {
        auto decoded = instruction->as<JSC::OpBelow>();
        if (!emitSignedOperand(visitorContext, "operand.op_below.dst", 20, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_below.lhs", 20, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_below.rhs", 20, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        return true;
    }
    case JSC::op_beloweq: {
        auto decoded = instruction->as<JSC::OpBeloweq>();
        if (!emitSignedOperand(visitorContext, "operand.op_beloweq.dst", 22, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_beloweq.lhs", 22, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_beloweq.rhs", 22, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        return true;
    }
    case JSC::op_mod: {
        auto decoded = instruction->as<JSC::OpMod>();
        if (!emitSignedOperand(visitorContext, "operand.op_mod.dst", 18, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_mod.lhs", 18, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_mod.rhs", 18, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        return true;
    }
    case JSC::op_pow: {
        auto decoded = instruction->as<JSC::OpPow>();
        if (!emitSignedOperand(visitorContext, "operand.op_pow.dst", 18, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_pow.lhs", 18, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_pow.rhs", 18, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        return true;
    }
    case JSC::op_urshift: {
        auto decoded = instruction->as<JSC::OpUrshift>();
        if (!emitSignedOperand(visitorContext, "operand.op_urshift.dst", 22, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_urshift.lhs", 22, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_urshift.rhs", 22, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        return true;
    }
    case JSC::op_add: {
        auto decoded = instruction->as<JSC::OpAdd>();
        if (!emitSignedOperand(visitorContext, "operand.op_add.dst", 18, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_add.lhs", 18, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_add.rhs", 18, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        return true;
    }
    case JSC::op_mul: {
        auto decoded = instruction->as<JSC::OpMul>();
        if (!emitSignedOperand(visitorContext, "operand.op_mul.dst", 18, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_mul.lhs", 18, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_mul.rhs", 18, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        return true;
    }
    case JSC::op_div: {
        auto decoded = instruction->as<JSC::OpDiv>();
        if (!emitSignedOperand(visitorContext, "operand.op_div.dst", 18, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_div.lhs", 18, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_div.rhs", 18, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        return true;
    }
    case JSC::op_sub: {
        auto decoded = instruction->as<JSC::OpSub>();
        if (!emitSignedOperand(visitorContext, "operand.op_sub.dst", 18, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_sub.lhs", 18, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_sub.rhs", 18, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        return true;
    }
    case JSC::op_bitand: {
        auto decoded = instruction->as<JSC::OpBitand>();
        if (!emitSignedOperand(visitorContext, "operand.op_bitand.dst", 21, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_bitand.lhs", 21, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_bitand.rhs", 21, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        return true;
    }
    case JSC::op_bitor: {
        auto decoded = instruction->as<JSC::OpBitor>();
        if (!emitSignedOperand(visitorContext, "operand.op_bitor.dst", 20, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_bitor.lhs", 20, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_bitor.rhs", 20, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        return true;
    }
    case JSC::op_bitxor: {
        auto decoded = instruction->as<JSC::OpBitxor>();
        if (!emitSignedOperand(visitorContext, "operand.op_bitxor.dst", 21, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_bitxor.lhs", 21, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_bitxor.rhs", 21, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        return true;
    }
    case JSC::op_lshift: {
        auto decoded = instruction->as<JSC::OpLshift>();
        if (!emitSignedOperand(visitorContext, "operand.op_lshift.dst", 21, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_lshift.lhs", 21, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_lshift.rhs", 21, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        return true;
    }
    case JSC::op_rshift: {
        auto decoded = instruction->as<JSC::OpRshift>();
        if (!emitSignedOperand(visitorContext, "operand.op_rshift.dst", 21, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_rshift.lhs", 21, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_rshift.rhs", 21, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        return true;
    }
    case JSC::op_eq_null: {
        auto decoded = instruction->as<JSC::OpEqNull>();
        if (!emitSignedOperand(visitorContext, "operand.op_eq_null.dst", 22, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_eq_null.operand", 26, 0, static_cast<int64_t>(decoded.m_operand.offset())))
            return false;
        return true;
    }
    case JSC::op_neq_null: {
        auto decoded = instruction->as<JSC::OpNeqNull>();
        if (!emitSignedOperand(visitorContext, "operand.op_neq_null.dst", 23, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_neq_null.operand", 27, 0, static_cast<int64_t>(decoded.m_operand.offset())))
            return false;
        return true;
    }
    case JSC::op_to_string: {
        auto decoded = instruction->as<JSC::OpToString>();
        if (!emitSignedOperand(visitorContext, "operand.op_to_string.dst", 24, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_to_string.operand", 28, 0, static_cast<int64_t>(decoded.m_operand.offset())))
            return false;
        return true;
    }
    case JSC::op_is_empty: {
        auto decoded = instruction->as<JSC::OpIsEmpty>();
        if (!emitSignedOperand(visitorContext, "operand.op_is_empty.dst", 23, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_is_empty.operand", 27, 0, static_cast<int64_t>(decoded.m_operand.offset())))
            return false;
        return true;
    }
    case JSC::op_typeof_is_undefined: {
        auto decoded = instruction->as<JSC::OpTypeofIsUndefined>();
        if (!emitSignedOperand(visitorContext, "operand.op_typeof_is_undefined.dst", 34, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_typeof_is_undefined.operand", 38, 0, static_cast<int64_t>(decoded.m_operand.offset())))
            return false;
        return true;
    }
    case JSC::op_typeof_is_object: {
        auto decoded = instruction->as<JSC::OpTypeofIsObject>();
        if (!emitSignedOperand(visitorContext, "operand.op_typeof_is_object.dst", 31, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_typeof_is_object.operand", 35, 0, static_cast<int64_t>(decoded.m_operand.offset())))
            return false;
        return true;
    }
    case JSC::op_typeof_is_function: {
        auto decoded = instruction->as<JSC::OpTypeofIsFunction>();
        if (!emitSignedOperand(visitorContext, "operand.op_typeof_is_function.dst", 33, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_typeof_is_function.operand", 37, 0, static_cast<int64_t>(decoded.m_operand.offset())))
            return false;
        return true;
    }
    case JSC::op_is_undefined_or_null: {
        auto decoded = instruction->as<JSC::OpIsUndefinedOrNull>();
        if (!emitSignedOperand(visitorContext, "operand.op_is_undefined_or_null.dst", 35, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_is_undefined_or_null.operand", 39, 0, static_cast<int64_t>(decoded.m_operand.offset())))
            return false;
        return true;
    }
    case JSC::op_is_boolean: {
        auto decoded = instruction->as<JSC::OpIsBoolean>();
        if (!emitSignedOperand(visitorContext, "operand.op_is_boolean.dst", 25, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_is_boolean.operand", 29, 0, static_cast<int64_t>(decoded.m_operand.offset())))
            return false;
        return true;
    }
    case JSC::op_is_number: {
        auto decoded = instruction->as<JSC::OpIsNumber>();
        if (!emitSignedOperand(visitorContext, "operand.op_is_number.dst", 24, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_is_number.operand", 28, 0, static_cast<int64_t>(decoded.m_operand.offset())))
            return false;
        return true;
    }
    case JSC::op_is_big_int: {
        auto decoded = instruction->as<JSC::OpIsBigInt>();
        if (!emitSignedOperand(visitorContext, "operand.op_is_big_int.dst", 25, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_is_big_int.operand", 29, 0, static_cast<int64_t>(decoded.m_operand.offset())))
            return false;
        return true;
    }
    case JSC::op_is_object: {
        auto decoded = instruction->as<JSC::OpIsObject>();
        if (!emitSignedOperand(visitorContext, "operand.op_is_object.dst", 24, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_is_object.operand", 28, 0, static_cast<int64_t>(decoded.m_operand.offset())))
            return false;
        return true;
    }
    case JSC::op_is_callable: {
        auto decoded = instruction->as<JSC::OpIsCallable>();
        if (!emitSignedOperand(visitorContext, "operand.op_is_callable.dst", 26, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_is_callable.operand", 30, 0, static_cast<int64_t>(decoded.m_operand.offset())))
            return false;
        return true;
    }
    case JSC::op_is_constructor: {
        auto decoded = instruction->as<JSC::OpIsConstructor>();
        if (!emitSignedOperand(visitorContext, "operand.op_is_constructor.dst", 29, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_is_constructor.operand", 33, 0, static_cast<int64_t>(decoded.m_operand.offset())))
            return false;
        return true;
    }
    case JSC::op_inc: {
        auto decoded = instruction->as<JSC::OpInc>();
        if (!emitSignedOperand(visitorContext, "operand.op_inc.srcDst", 21, 2, static_cast<int64_t>(decoded.m_srcDst.offset())))
            return false;
        return true;
    }
    case JSC::op_dec: {
        auto decoded = instruction->as<JSC::OpDec>();
        if (!emitSignedOperand(visitorContext, "operand.op_dec.srcDst", 21, 2, static_cast<int64_t>(decoded.m_srcDst.offset())))
            return false;
        return true;
    }
    case JSC::op_negate: {
        auto decoded = instruction->as<JSC::OpNegate>();
        if (!emitSignedOperand(visitorContext, "operand.op_negate.dst", 21, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_negate.operand", 25, 0, static_cast<int64_t>(decoded.m_operand.offset())))
            return false;
        return true;
    }
    case JSC::op_not: {
        auto decoded = instruction->as<JSC::OpNot>();
        if (!emitSignedOperand(visitorContext, "operand.op_not.dst", 18, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_not.operand", 22, 0, static_cast<int64_t>(decoded.m_operand.offset())))
            return false;
        return true;
    }
    case JSC::op_identity_with_profile: {
        auto decoded = instruction->as<JSC::OpIdentityWithProfile>();
        if (!emitSignedOperand(visitorContext, "operand.op_identity_with_profile.srcDst", 39, 2, static_cast<int64_t>(decoded.m_srcDst.offset())))
            return false;
        return true;
    }
    case JSC::op_typeof: {
        auto decoded = instruction->as<JSC::OpTypeof>();
        if (!emitSignedOperand(visitorContext, "operand.op_typeof.dst", 21, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_typeof.value", 23, 0, static_cast<int64_t>(decoded.m_value.offset())))
            return false;
        return true;
    }
    case JSC::op_is_cell_with_type: {
        auto decoded = instruction->as<JSC::OpIsCellWithType>();
        if (!emitSignedOperand(visitorContext, "operand.op_is_cell_with_type.dst", 32, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_is_cell_with_type.operand", 36, 0, static_cast<int64_t>(decoded.m_operand.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_is_cell_with_type.type", 33, 22, static_cast<uint64_t>(decoded.m_type)))
            return false;
        return true;
    }
    case JSC::op_has_structure_with_flags: {
        auto decoded = instruction->as<JSC::OpHasStructureWithFlags>();
        if (!emitSignedOperand(visitorContext, "operand.op_has_structure_with_flags.dst", 39, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_has_structure_with_flags.operand", 43, 0, static_cast<int64_t>(decoded.m_operand.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_has_structure_with_flags.flags", 41, 17, static_cast<uint64_t>(decoded.m_flags)))
            return false;
        return true;
    }
    case JSC::op_to_number: {
        auto decoded = instruction->as<JSC::OpToNumber>();
        if (!emitSignedOperand(visitorContext, "operand.op_to_number.dst", 24, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_to_number.operand", 28, 0, static_cast<int64_t>(decoded.m_operand.offset())))
            return false;
        return true;
    }
    case JSC::op_to_numeric: {
        auto decoded = instruction->as<JSC::OpToNumeric>();
        if (!emitSignedOperand(visitorContext, "operand.op_to_numeric.dst", 25, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_to_numeric.operand", 29, 0, static_cast<int64_t>(decoded.m_operand.offset())))
            return false;
        return true;
    }
    case JSC::op_bitnot: {
        auto decoded = instruction->as<JSC::OpBitnot>();
        if (!emitSignedOperand(visitorContext, "operand.op_bitnot.dst", 21, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_bitnot.operand", 25, 0, static_cast<int64_t>(decoded.m_operand.offset())))
            return false;
        return true;
    }
    case JSC::op_unsigned: {
        auto decoded = instruction->as<JSC::OpUnsigned>();
        if (!emitSignedOperand(visitorContext, "operand.op_unsigned.dst", 23, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_unsigned.operand", 27, 0, static_cast<int64_t>(decoded.m_operand.offset())))
            return false;
        return true;
    }
