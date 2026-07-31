    case JSC::op_catch: {
        auto decoded = instruction->as<JSC::OpCatch>();
        if (!emitSignedOperand(visitorContext, "operand.op_catch.exception", 26, 1, static_cast<int64_t>(decoded.m_exception.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_catch.thrownValue", 28, 1, static_cast<int64_t>(decoded.m_thrownValue.offset())))
            return false;
        return true;
    }
    case JSC::op_throw: {
        auto decoded = instruction->as<JSC::OpThrow>();
        if (!emitSignedOperand(visitorContext, "operand.op_throw.value", 22, 0, static_cast<int64_t>(decoded.m_value.offset())))
            return false;
        return true;
    }
    case JSC::op_throw_static_error: {
        auto decoded = instruction->as<JSC::OpThrowStaticError>();
        if (!emitSignedOperand(visitorContext, "operand.op_throw_static_error.message", 37, 0, static_cast<int64_t>(decoded.m_message.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_throw_static_error.errorType", 39, 22, static_cast<uint64_t>(decoded.m_errorType)))
            return false;
        return true;
    }
    case JSC::op_unreachable: {
        return true;
    }
    case JSC::op_check_traps: {
        return true;
    }
