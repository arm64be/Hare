    case JSC::op_jneq_ptr: {
        auto decoded = instruction->as<JSC::OpJneqPtr>();
        if (!emitSignedOperand(visitorContext, "operand.op_jneq_ptr.value", 25, 0, static_cast<int64_t>(decoded.m_value.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jneq_ptr.specialPointer", 34, 0, static_cast<int64_t>(decoded.m_specialPointer.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jneq_ptr.targetLabel", 31, 5, static_cast<int64_t>(decoded.m_targetLabel.target() ? decoded.m_targetLabel.target() : block.outOfLineJumpOffset(instructionRef))))
            return false;
        return true;
    }
    case JSC::op_jmp: {
        auto decoded = instruction->as<JSC::OpJmp>();
        if (!emitSignedOperand(visitorContext, "operand.op_jmp.targetLabel", 26, 5, static_cast<int64_t>(decoded.m_targetLabel.target() ? decoded.m_targetLabel.target() : block.outOfLineJumpOffset(instructionRef))))
            return false;
        return true;
    }
    case JSC::op_jtrue: {
        auto decoded = instruction->as<JSC::OpJtrue>();
        if (!emitSignedOperand(visitorContext, "operand.op_jtrue.condition", 26, 0, static_cast<int64_t>(decoded.m_condition.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jtrue.targetLabel", 28, 5, static_cast<int64_t>(decoded.m_targetLabel.target() ? decoded.m_targetLabel.target() : block.outOfLineJumpOffset(instructionRef))))
            return false;
        return true;
    }
    case JSC::op_jfalse: {
        auto decoded = instruction->as<JSC::OpJfalse>();
        if (!emitSignedOperand(visitorContext, "operand.op_jfalse.condition", 27, 0, static_cast<int64_t>(decoded.m_condition.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jfalse.targetLabel", 29, 5, static_cast<int64_t>(decoded.m_targetLabel.target() ? decoded.m_targetLabel.target() : block.outOfLineJumpOffset(instructionRef))))
            return false;
        return true;
    }
    case JSC::op_jeq_null: {
        auto decoded = instruction->as<JSC::OpJeqNull>();
        if (!emitSignedOperand(visitorContext, "operand.op_jeq_null.value", 25, 0, static_cast<int64_t>(decoded.m_value.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jeq_null.targetLabel", 31, 5, static_cast<int64_t>(decoded.m_targetLabel.target() ? decoded.m_targetLabel.target() : block.outOfLineJumpOffset(instructionRef))))
            return false;
        return true;
    }
    case JSC::op_jneq_null: {
        auto decoded = instruction->as<JSC::OpJneqNull>();
        if (!emitSignedOperand(visitorContext, "operand.op_jneq_null.value", 26, 0, static_cast<int64_t>(decoded.m_value.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jneq_null.targetLabel", 32, 5, static_cast<int64_t>(decoded.m_targetLabel.target() ? decoded.m_targetLabel.target() : block.outOfLineJumpOffset(instructionRef))))
            return false;
        return true;
    }
    case JSC::op_jundefined_or_null: {
        auto decoded = instruction->as<JSC::OpJundefinedOrNull>();
        if (!emitSignedOperand(visitorContext, "operand.op_jundefined_or_null.value", 35, 0, static_cast<int64_t>(decoded.m_value.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jundefined_or_null.targetLabel", 41, 5, static_cast<int64_t>(decoded.m_targetLabel.target() ? decoded.m_targetLabel.target() : block.outOfLineJumpOffset(instructionRef))))
            return false;
        return true;
    }
    case JSC::op_jnundefined_or_null: {
        auto decoded = instruction->as<JSC::OpJnundefinedOrNull>();
        if (!emitSignedOperand(visitorContext, "operand.op_jnundefined_or_null.value", 36, 0, static_cast<int64_t>(decoded.m_value.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jnundefined_or_null.targetLabel", 42, 5, static_cast<int64_t>(decoded.m_targetLabel.target() ? decoded.m_targetLabel.target() : block.outOfLineJumpOffset(instructionRef))))
            return false;
        return true;
    }
    case JSC::op_jeq_ptr: {
        auto decoded = instruction->as<JSC::OpJeqPtr>();
        if (!emitSignedOperand(visitorContext, "operand.op_jeq_ptr.value", 24, 0, static_cast<int64_t>(decoded.m_value.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jeq_ptr.specialPointer", 33, 0, static_cast<int64_t>(decoded.m_specialPointer.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jeq_ptr.targetLabel", 30, 5, static_cast<int64_t>(decoded.m_targetLabel.target() ? decoded.m_targetLabel.target() : block.outOfLineJumpOffset(instructionRef))))
            return false;
        return true;
    }
    case JSC::op_jeq: {
        auto decoded = instruction->as<JSC::OpJeq>();
        if (!emitSignedOperand(visitorContext, "operand.op_jeq.lhs", 18, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jeq.rhs", 18, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jeq.targetLabel", 26, 5, static_cast<int64_t>(decoded.m_targetLabel.target() ? decoded.m_targetLabel.target() : block.outOfLineJumpOffset(instructionRef))))
            return false;
        return true;
    }
    case JSC::op_jstricteq: {
        auto decoded = instruction->as<JSC::OpJstricteq>();
        if (!emitSignedOperand(visitorContext, "operand.op_jstricteq.lhs", 24, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jstricteq.rhs", 24, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jstricteq.targetLabel", 32, 5, static_cast<int64_t>(decoded.m_targetLabel.target() ? decoded.m_targetLabel.target() : block.outOfLineJumpOffset(instructionRef))))
            return false;
        return true;
    }
    case JSC::op_jneq: {
        auto decoded = instruction->as<JSC::OpJneq>();
        if (!emitSignedOperand(visitorContext, "operand.op_jneq.lhs", 19, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jneq.rhs", 19, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jneq.targetLabel", 27, 5, static_cast<int64_t>(decoded.m_targetLabel.target() ? decoded.m_targetLabel.target() : block.outOfLineJumpOffset(instructionRef))))
            return false;
        return true;
    }
    case JSC::op_jnstricteq: {
        auto decoded = instruction->as<JSC::OpJnstricteq>();
        if (!emitSignedOperand(visitorContext, "operand.op_jnstricteq.lhs", 25, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jnstricteq.rhs", 25, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jnstricteq.targetLabel", 33, 5, static_cast<int64_t>(decoded.m_targetLabel.target() ? decoded.m_targetLabel.target() : block.outOfLineJumpOffset(instructionRef))))
            return false;
        return true;
    }
    case JSC::op_jless: {
        auto decoded = instruction->as<JSC::OpJless>();
        if (!emitSignedOperand(visitorContext, "operand.op_jless.lhs", 20, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jless.rhs", 20, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jless.targetLabel", 28, 5, static_cast<int64_t>(decoded.m_targetLabel.target() ? decoded.m_targetLabel.target() : block.outOfLineJumpOffset(instructionRef))))
            return false;
        return true;
    }
    case JSC::op_jlesseq: {
        auto decoded = instruction->as<JSC::OpJlesseq>();
        if (!emitSignedOperand(visitorContext, "operand.op_jlesseq.lhs", 22, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jlesseq.rhs", 22, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jlesseq.targetLabel", 30, 5, static_cast<int64_t>(decoded.m_targetLabel.target() ? decoded.m_targetLabel.target() : block.outOfLineJumpOffset(instructionRef))))
            return false;
        return true;
    }
    case JSC::op_jgreater: {
        auto decoded = instruction->as<JSC::OpJgreater>();
        if (!emitSignedOperand(visitorContext, "operand.op_jgreater.lhs", 23, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jgreater.rhs", 23, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jgreater.targetLabel", 31, 5, static_cast<int64_t>(decoded.m_targetLabel.target() ? decoded.m_targetLabel.target() : block.outOfLineJumpOffset(instructionRef))))
            return false;
        return true;
    }
    case JSC::op_jgreatereq: {
        auto decoded = instruction->as<JSC::OpJgreatereq>();
        if (!emitSignedOperand(visitorContext, "operand.op_jgreatereq.lhs", 25, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jgreatereq.rhs", 25, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jgreatereq.targetLabel", 33, 5, static_cast<int64_t>(decoded.m_targetLabel.target() ? decoded.m_targetLabel.target() : block.outOfLineJumpOffset(instructionRef))))
            return false;
        return true;
    }
    case JSC::op_jnless: {
        auto decoded = instruction->as<JSC::OpJnless>();
        if (!emitSignedOperand(visitorContext, "operand.op_jnless.lhs", 21, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jnless.rhs", 21, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jnless.targetLabel", 29, 5, static_cast<int64_t>(decoded.m_targetLabel.target() ? decoded.m_targetLabel.target() : block.outOfLineJumpOffset(instructionRef))))
            return false;
        return true;
    }
    case JSC::op_jnlesseq: {
        auto decoded = instruction->as<JSC::OpJnlesseq>();
        if (!emitSignedOperand(visitorContext, "operand.op_jnlesseq.lhs", 23, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jnlesseq.rhs", 23, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jnlesseq.targetLabel", 31, 5, static_cast<int64_t>(decoded.m_targetLabel.target() ? decoded.m_targetLabel.target() : block.outOfLineJumpOffset(instructionRef))))
            return false;
        return true;
    }
    case JSC::op_jngreater: {
        auto decoded = instruction->as<JSC::OpJngreater>();
        if (!emitSignedOperand(visitorContext, "operand.op_jngreater.lhs", 24, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jngreater.rhs", 24, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jngreater.targetLabel", 32, 5, static_cast<int64_t>(decoded.m_targetLabel.target() ? decoded.m_targetLabel.target() : block.outOfLineJumpOffset(instructionRef))))
            return false;
        return true;
    }
    case JSC::op_jngreatereq: {
        auto decoded = instruction->as<JSC::OpJngreatereq>();
        if (!emitSignedOperand(visitorContext, "operand.op_jngreatereq.lhs", 26, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jngreatereq.rhs", 26, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jngreatereq.targetLabel", 34, 5, static_cast<int64_t>(decoded.m_targetLabel.target() ? decoded.m_targetLabel.target() : block.outOfLineJumpOffset(instructionRef))))
            return false;
        return true;
    }
    case JSC::op_jbelow: {
        auto decoded = instruction->as<JSC::OpJbelow>();
        if (!emitSignedOperand(visitorContext, "operand.op_jbelow.lhs", 21, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jbelow.rhs", 21, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jbelow.targetLabel", 29, 5, static_cast<int64_t>(decoded.m_targetLabel.target() ? decoded.m_targetLabel.target() : block.outOfLineJumpOffset(instructionRef))))
            return false;
        return true;
    }
    case JSC::op_jbeloweq: {
        auto decoded = instruction->as<JSC::OpJbeloweq>();
        if (!emitSignedOperand(visitorContext, "operand.op_jbeloweq.lhs", 23, 0, static_cast<int64_t>(decoded.m_lhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jbeloweq.rhs", 23, 0, static_cast<int64_t>(decoded.m_rhs.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_jbeloweq.targetLabel", 31, 5, static_cast<int64_t>(decoded.m_targetLabel.target() ? decoded.m_targetLabel.target() : block.outOfLineJumpOffset(instructionRef))))
            return false;
        return true;
    }
    case JSC::op_switch_imm: {
        auto decoded = instruction->as<JSC::OpSwitchImm>();
        if (!emitUnsignedOperand(visitorContext, "operand.op_switch_imm.tableIndex", 32, 11, static_cast<uint64_t>(decoded.m_tableIndex)))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_switch_imm.scrutinee", 31, 0, static_cast<int64_t>(decoded.m_scrutinee.offset())))
            return false;
        return true;
    }
    case JSC::op_switch_char: {
        auto decoded = instruction->as<JSC::OpSwitchChar>();
        if (!emitUnsignedOperand(visitorContext, "operand.op_switch_char.tableIndex", 33, 11, static_cast<uint64_t>(decoded.m_tableIndex)))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_switch_char.scrutinee", 32, 0, static_cast<int64_t>(decoded.m_scrutinee.offset())))
            return false;
        return true;
    }
    case JSC::op_switch_string: {
        auto decoded = instruction->as<JSC::OpSwitchString>();
        if (!emitUnsignedOperand(visitorContext, "operand.op_switch_string.tableIndex", 35, 11, static_cast<uint64_t>(decoded.m_tableIndex)))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_switch_string.scrutinee", 34, 0, static_cast<int64_t>(decoded.m_scrutinee.offset())))
            return false;
        return true;
    }
    case JSC::op_ret: {
        auto decoded = instruction->as<JSC::OpRet>();
        if (!emitSignedOperand(visitorContext, "operand.op_ret.value", 20, 0, static_cast<int64_t>(decoded.m_value.offset())))
            return false;
        return true;
    }
    case JSC::op_enter: {
        return true;
    }
    case JSC::op_mov: {
        auto decoded = instruction->as<JSC::OpMov>();
        if (!emitSignedOperand(visitorContext, "operand.op_mov.dst", 18, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_mov.src", 18, 0, static_cast<int64_t>(decoded.m_src.offset())))
            return false;
        return true;
    }
