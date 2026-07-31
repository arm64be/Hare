    case JSC::op_call_direct_eval: {
        auto decoded = instruction->as<JSC::OpCallDirectEval>();
        if (!emitSignedOperand(visitorContext, "operand.op_call_direct_eval.dst", 31, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_call_direct_eval.callee", 34, 0, static_cast<int64_t>(decoded.m_callee.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_call_direct_eval.argc", 32, 6, static_cast<uint64_t>(decoded.m_argc)))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_call_direct_eval.argv", 32, 8, static_cast<uint64_t>(decoded.m_argv)))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_call_direct_eval.thisValue", 37, 0, static_cast<int64_t>(decoded.m_thisValue.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_call_direct_eval.scope", 33, 0, static_cast<int64_t>(decoded.m_scope.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_call_direct_eval.lexicallyScopedFeatures", 51, 15, static_cast<uint64_t>(decoded.m_lexicallyScopedFeatures)))
            return false;
        return true;
    }
    case JSC::op_resolve_scope_for_hoisting_func_decl_in_eval: {
        auto decoded = instruction->as<JSC::OpResolveScopeForHoistingFuncDeclInEval>();
        if (!emitSignedOperand(visitorContext, "operand.op_resolve_scope_for_hoisting_func_decl_in_eval.dst", 59, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_resolve_scope_for_hoisting_func_decl_in_eval.scope", 61, 0, static_cast<int64_t>(decoded.m_scope.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_resolve_scope_for_hoisting_func_decl_in_eval.property", 64, 9, static_cast<uint64_t>(decoded.m_property)))
            return false;
        return true;
    }
