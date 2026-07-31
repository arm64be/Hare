    case JSC::op_tail_call_varargs: {
        auto decoded = instruction->as<JSC::OpTailCallVarargs>();
        if (!emitSignedOperand(visitorContext, "operand.op_tail_call_varargs.dst", 32, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_tail_call_varargs.callee", 35, 0, static_cast<int64_t>(decoded.m_callee.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_tail_call_varargs.thisValue", 38, 0, static_cast<int64_t>(decoded.m_thisValue.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_tail_call_varargs.arguments", 38, 0, static_cast<int64_t>(decoded.m_arguments.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_tail_call_varargs.firstVarArg", 40, 7, static_cast<int64_t>(decoded.m_firstVarArg)))
            return false;
        return true;
    }
    case JSC::op_call_varargs: {
        auto decoded = instruction->as<JSC::OpCallVarargs>();
        if (!emitSignedOperand(visitorContext, "operand.op_call_varargs.dst", 27, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_call_varargs.callee", 30, 0, static_cast<int64_t>(decoded.m_callee.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_call_varargs.thisValue", 33, 0, static_cast<int64_t>(decoded.m_thisValue.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_call_varargs.arguments", 33, 0, static_cast<int64_t>(decoded.m_arguments.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_call_varargs.firstVarArg", 35, 7, static_cast<int64_t>(decoded.m_firstVarArg)))
            return false;
        return true;
    }
    case JSC::op_iterator_next: {
        auto decoded = instruction->as<JSC::OpIteratorNext>();
        if (!emitSignedOperand(visitorContext, "operand.op_iterator_next.done", 29, 1, static_cast<int64_t>(decoded.m_done.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_iterator_next.value", 30, 1, static_cast<int64_t>(decoded.m_value.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_iterator_next.iterable", 33, 0, static_cast<int64_t>(decoded.m_iterable.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_iterator_next.next", 29, 0, static_cast<int64_t>(decoded.m_next.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_iterator_next.iterator", 33, 0, static_cast<int64_t>(decoded.m_iterator.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_iterator_next.stackOffset", 36, 13, static_cast<uint64_t>(decoded.m_stackOffset)))
            return false;
        return true;
    }
    case JSC::op_construct_varargs: {
        auto decoded = instruction->as<JSC::OpConstructVarargs>();
        if (!emitSignedOperand(visitorContext, "operand.op_construct_varargs.dst", 32, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_construct_varargs.callee", 35, 0, static_cast<int64_t>(decoded.m_callee.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_construct_varargs.thisValue", 38, 0, static_cast<int64_t>(decoded.m_thisValue.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_construct_varargs.arguments", 38, 0, static_cast<int64_t>(decoded.m_arguments.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_construct_varargs.firstVarArg", 40, 7, static_cast<int64_t>(decoded.m_firstVarArg)))
            return false;
        return true;
    }
    case JSC::op_super_construct_varargs: {
        auto decoded = instruction->as<JSC::OpSuperConstructVarargs>();
        if (!emitSignedOperand(visitorContext, "operand.op_super_construct_varargs.dst", 38, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_super_construct_varargs.callee", 41, 0, static_cast<int64_t>(decoded.m_callee.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_super_construct_varargs.thisValue", 44, 0, static_cast<int64_t>(decoded.m_thisValue.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_super_construct_varargs.arguments", 44, 0, static_cast<int64_t>(decoded.m_arguments.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_super_construct_varargs.firstVarArg", 46, 7, static_cast<int64_t>(decoded.m_firstVarArg)))
            return false;
        return true;
    }
    case JSC::op_iterator_open: {
        auto decoded = instruction->as<JSC::OpIteratorOpen>();
        if (!emitSignedOperand(visitorContext, "operand.op_iterator_open.iterator", 33, 2, static_cast<int64_t>(decoded.m_iterator.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_iterator_open.next", 29, 1, static_cast<int64_t>(decoded.m_next.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_iterator_open.symbolIterator", 39, 0, static_cast<int64_t>(decoded.m_symbolIterator.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_iterator_open.iterable", 33, 0, static_cast<int64_t>(decoded.m_iterable.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_iterator_open.stackOffset", 36, 13, static_cast<uint64_t>(decoded.m_stackOffset)))
            return false;
        return true;
    }
    case JSC::op_async_iterator_open: {
        auto decoded = instruction->as<JSC::OpAsyncIteratorOpen>();
        if (!emitSignedOperand(visitorContext, "operand.op_async_iterator_open.iterator", 39, 2, static_cast<int64_t>(decoded.m_iterator.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_async_iterator_open.next", 35, 1, static_cast<int64_t>(decoded.m_next.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_async_iterator_open.symbolIterator", 45, 0, static_cast<int64_t>(decoded.m_symbolIterator.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_async_iterator_open.iterable", 39, 0, static_cast<int64_t>(decoded.m_iterable.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_async_iterator_open.stackOffset", 42, 13, static_cast<uint64_t>(decoded.m_stackOffset)))
            return false;
        return true;
    }
    case JSC::op_construct: {
        auto decoded = instruction->as<JSC::OpConstruct>();
        if (!emitSignedOperand(visitorContext, "operand.op_construct.dst", 24, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_construct.callee", 27, 0, static_cast<int64_t>(decoded.m_callee.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_construct.argc", 25, 6, static_cast<uint64_t>(decoded.m_argc)))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_construct.argv", 25, 8, static_cast<uint64_t>(decoded.m_argv)))
            return false;
        return true;
    }
    case JSC::op_super_construct: {
        auto decoded = instruction->as<JSC::OpSuperConstruct>();
        if (!emitSignedOperand(visitorContext, "operand.op_super_construct.dst", 30, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_super_construct.callee", 33, 0, static_cast<int64_t>(decoded.m_callee.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_super_construct.argc", 31, 6, static_cast<uint64_t>(decoded.m_argc)))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_super_construct.argv", 31, 8, static_cast<uint64_t>(decoded.m_argv)))
            return false;
        return true;
    }
    case JSC::op_tail_call: {
        auto decoded = instruction->as<JSC::OpTailCall>();
        if (!emitSignedOperand(visitorContext, "operand.op_tail_call.dst", 24, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_tail_call.callee", 27, 0, static_cast<int64_t>(decoded.m_callee.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_tail_call.argc", 25, 6, static_cast<uint64_t>(decoded.m_argc)))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_tail_call.argv", 25, 8, static_cast<uint64_t>(decoded.m_argv)))
            return false;
        return true;
    }
    case JSC::op_create_generator: {
        auto decoded = instruction->as<JSC::OpCreateGenerator>();
        if (!emitSignedOperand(visitorContext, "operand.op_create_generator.dst", 31, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_create_generator.callee", 34, 0, static_cast<int64_t>(decoded.m_callee.offset())))
            return false;
        return true;
    }
    case JSC::op_create_async_generator: {
        auto decoded = instruction->as<JSC::OpCreateAsyncGenerator>();
        if (!emitSignedOperand(visitorContext, "operand.op_create_async_generator.dst", 37, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_create_async_generator.callee", 40, 0, static_cast<int64_t>(decoded.m_callee.offset())))
            return false;
        return true;
    }
    case JSC::op_create_promise: {
        auto decoded = instruction->as<JSC::OpCreatePromise>();
        if (!emitSignedOperand(visitorContext, "operand.op_create_promise.dst", 29, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_create_promise.callee", 32, 0, static_cast<int64_t>(decoded.m_callee.offset())))
            return false;
        return true;
    }
    case JSC::op_call: {
        auto decoded = instruction->as<JSC::OpCall>();
        if (!emitSignedOperand(visitorContext, "operand.op_call.dst", 19, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_call.callee", 22, 0, static_cast<int64_t>(decoded.m_callee.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_call.argc", 20, 6, static_cast<uint64_t>(decoded.m_argc)))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_call.argv", 20, 8, static_cast<uint64_t>(decoded.m_argv)))
            return false;
        return true;
    }
    case JSC::op_call_ignore_result: {
        auto decoded = instruction->as<JSC::OpCallIgnoreResult>();
        if (!emitSignedOperand(visitorContext, "operand.op_call_ignore_result.callee", 36, 0, static_cast<int64_t>(decoded.m_callee.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_call_ignore_result.argc", 34, 6, static_cast<uint64_t>(decoded.m_argc)))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_call_ignore_result.argv", 34, 8, static_cast<uint64_t>(decoded.m_argv)))
            return false;
        return true;
    }
    case JSC::op_async_iterator_next: {
        auto decoded = instruction->as<JSC::OpAsyncIteratorNext>();
        if (!emitSignedOperand(visitorContext, "operand.op_async_iterator_next.dst", 34, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_async_iterator_next.next", 35, 0, static_cast<int64_t>(decoded.m_next.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_async_iterator_next.iterator", 39, 0, static_cast<int64_t>(decoded.m_iterator.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_async_iterator_next.driver", 37, 0, static_cast<int64_t>(decoded.m_driver.offset())))
            return false;
        if (!emitBooleanOperand(visitorContext, "operand.op_async_iterator_next.hasValue", 39, 23, decoded.m_hasValue))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_async_iterator_next.stackOffset", 42, 13, static_cast<uint64_t>(decoded.m_stackOffset)))
            return false;
        return true;
    }
    case JSC::op_resolve_scope: {
        auto decoded = instruction->as<JSC::OpResolveScope>();
        if (!emitSignedOperand(visitorContext, "operand.op_resolve_scope.dst", 28, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_resolve_scope.scope", 30, 0, static_cast<int64_t>(decoded.m_scope.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_resolve_scope.var", 28, 9, static_cast<uint64_t>(decoded.m_var)))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_resolve_scope.resolveType", 36, 22, static_cast<uint64_t>(decoded.m_resolveType)))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_resolve_scope.localScopeDepth", 40, 18, static_cast<uint64_t>(decoded.m_localScopeDepth)))
            return false;
        return true;
    }
    case JSC::op_get_from_scope: {
        auto decoded = instruction->as<JSC::OpGetFromScope>();
        if (!emitSignedOperand(visitorContext, "operand.op_get_from_scope.dst", 29, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_get_from_scope.scope", 31, 0, static_cast<int64_t>(decoded.m_scope.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_get_from_scope.var", 29, 9, static_cast<uint64_t>(decoded.m_var)))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_get_from_scope.getPutInfo", 36, 22, decoded.m_getPutInfo.operand()))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_get_from_scope.localScopeDepth", 41, 18, static_cast<uint64_t>(decoded.m_localScopeDepth)))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_get_from_scope.offset", 32, 19, static_cast<uint64_t>(decoded.m_offset)))
            return false;
        return true;
    }
    case JSC::op_put_to_scope: {
        auto decoded = instruction->as<JSC::OpPutToScope>();
        if (!emitSignedOperand(visitorContext, "operand.op_put_to_scope.scope", 29, 0, static_cast<int64_t>(decoded.m_scope.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_put_to_scope.var", 27, 9, static_cast<uint64_t>(decoded.m_var)))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_put_to_scope.value", 29, 0, static_cast<int64_t>(decoded.m_value.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_put_to_scope.getPutInfo", 34, 22, decoded.m_getPutInfo.operand()))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_put_to_scope.symbolTableOrScopeDepth", 47, 20, decoded.m_symbolTableOrScopeDepth.raw()))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_put_to_scope.offset", 30, 19, static_cast<uint64_t>(decoded.m_offset)))
            return false;
        return true;
    }
    case JSC::op_create_this: {
        auto decoded = instruction->as<JSC::OpCreateThis>();
        if (!emitSignedOperand(visitorContext, "operand.op_create_this.dst", 26, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_create_this.callee", 29, 0, static_cast<int64_t>(decoded.m_callee.offset())))
            return false;
        return true;
    }
    case JSC::op_to_this: {
        auto decoded = instruction->as<JSC::OpToThis>();
        if (!emitSignedOperand(visitorContext, "operand.op_to_this.srcDst", 25, 2, static_cast<int64_t>(decoded.m_srcDst.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_to_this.ecmaMode", 27, 22, decoded.m_ecmaMode.value()))
            return false;
        return true;
    }
    case JSC::op_get_argument: {
        auto decoded = instruction->as<JSC::OpGetArgument>();
        if (!emitSignedOperand(visitorContext, "operand.op_get_argument.dst", 27, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_get_argument.index", 29, 7, static_cast<int64_t>(decoded.m_index)))
            return false;
        return true;
    }
    case JSC::op_get_from_arguments: {
        auto decoded = instruction->as<JSC::OpGetFromArguments>();
        if (!emitSignedOperand(visitorContext, "operand.op_get_from_arguments.dst", 33, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_get_from_arguments.arguments", 39, 0, static_cast<int64_t>(decoded.m_arguments.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_get_from_arguments.index", 35, 14, static_cast<uint64_t>(decoded.m_index)))
            return false;
        return true;
    }
    case JSC::op_new_func: {
        auto decoded = instruction->as<JSC::OpNewFunc>();
        if (!emitSignedOperand(visitorContext, "operand.op_new_func.dst", 23, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_new_func.scope", 25, 0, static_cast<int64_t>(decoded.m_scope.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_new_func.functionDecl", 32, 10, static_cast<uint64_t>(decoded.m_functionDecl)))
            return false;
        return true;
    }
    case JSC::op_new_func_exp: {
        auto decoded = instruction->as<JSC::OpNewFuncExp>();
        if (!emitSignedOperand(visitorContext, "operand.op_new_func_exp.dst", 27, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_new_func_exp.scope", 29, 0, static_cast<int64_t>(decoded.m_scope.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_new_func_exp.functionDecl", 36, 10, static_cast<uint64_t>(decoded.m_functionDecl)))
            return false;
        return true;
    }
    case JSC::op_new_generator_func: {
        auto decoded = instruction->as<JSC::OpNewGeneratorFunc>();
        if (!emitSignedOperand(visitorContext, "operand.op_new_generator_func.dst", 33, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_new_generator_func.scope", 35, 0, static_cast<int64_t>(decoded.m_scope.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_new_generator_func.functionDecl", 42, 10, static_cast<uint64_t>(decoded.m_functionDecl)))
            return false;
        return true;
    }
    case JSC::op_new_generator_func_exp: {
        auto decoded = instruction->as<JSC::OpNewGeneratorFuncExp>();
        if (!emitSignedOperand(visitorContext, "operand.op_new_generator_func_exp.dst", 37, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_new_generator_func_exp.scope", 39, 0, static_cast<int64_t>(decoded.m_scope.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_new_generator_func_exp.functionDecl", 46, 10, static_cast<uint64_t>(decoded.m_functionDecl)))
            return false;
        return true;
    }
    case JSC::op_new_async_func: {
        auto decoded = instruction->as<JSC::OpNewAsyncFunc>();
        if (!emitSignedOperand(visitorContext, "operand.op_new_async_func.dst", 29, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_new_async_func.scope", 31, 0, static_cast<int64_t>(decoded.m_scope.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_new_async_func.functionDecl", 38, 10, static_cast<uint64_t>(decoded.m_functionDecl)))
            return false;
        return true;
    }
    case JSC::op_new_async_func_exp: {
        auto decoded = instruction->as<JSC::OpNewAsyncFuncExp>();
        if (!emitSignedOperand(visitorContext, "operand.op_new_async_func_exp.dst", 33, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_new_async_func_exp.scope", 35, 0, static_cast<int64_t>(decoded.m_scope.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_new_async_func_exp.functionDecl", 42, 10, static_cast<uint64_t>(decoded.m_functionDecl)))
            return false;
        return true;
    }
    case JSC::op_new_async_generator_func: {
        auto decoded = instruction->as<JSC::OpNewAsyncGeneratorFunc>();
        if (!emitSignedOperand(visitorContext, "operand.op_new_async_generator_func.dst", 39, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_new_async_generator_func.scope", 41, 0, static_cast<int64_t>(decoded.m_scope.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_new_async_generator_func.functionDecl", 48, 10, static_cast<uint64_t>(decoded.m_functionDecl)))
            return false;
        return true;
    }
    case JSC::op_new_async_generator_func_exp: {
        auto decoded = instruction->as<JSC::OpNewAsyncGeneratorFuncExp>();
        if (!emitSignedOperand(visitorContext, "operand.op_new_async_generator_func_exp.dst", 43, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_new_async_generator_func_exp.scope", 45, 0, static_cast<int64_t>(decoded.m_scope.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_new_async_generator_func_exp.functionDecl", 52, 10, static_cast<uint64_t>(decoded.m_functionDecl)))
            return false;
        return true;
    }
    case JSC::op_set_function_name: {
        auto decoded = instruction->as<JSC::OpSetFunctionName>();
        if (!emitSignedOperand(visitorContext, "operand.op_set_function_name.function", 37, 0, static_cast<int64_t>(decoded.m_function.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_set_function_name.name", 33, 0, static_cast<int64_t>(decoded.m_name.offset())))
            return false;
        return true;
    }
    case JSC::op_put_to_arguments: {
        auto decoded = instruction->as<JSC::OpPutToArguments>();
        if (!emitSignedOperand(visitorContext, "operand.op_put_to_arguments.arguments", 37, 0, static_cast<int64_t>(decoded.m_arguments.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_put_to_arguments.index", 33, 14, static_cast<uint64_t>(decoded.m_index)))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_put_to_arguments.value", 33, 0, static_cast<int64_t>(decoded.m_value.offset())))
            return false;
        return true;
    }
    case JSC::op_push_with_scope: {
        auto decoded = instruction->as<JSC::OpPushWithScope>();
        if (!emitSignedOperand(visitorContext, "operand.op_push_with_scope.dst", 30, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_push_with_scope.currentScope", 39, 0, static_cast<int64_t>(decoded.m_currentScope.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_push_with_scope.newScope", 35, 0, static_cast<int64_t>(decoded.m_newScope.offset())))
            return false;
        return true;
    }
    case JSC::op_create_lexical_environment: {
        auto decoded = instruction->as<JSC::OpCreateLexicalEnvironment>();
        if (!emitSignedOperand(visitorContext, "operand.op_create_lexical_environment.dst", 41, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_create_lexical_environment.scope", 43, 0, static_cast<int64_t>(decoded.m_scope.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_create_lexical_environment.symbolTable", 49, 0, static_cast<int64_t>(decoded.m_symbolTable.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_create_lexical_environment.initialValue", 50, 0, static_cast<int64_t>(decoded.m_initialValue.offset())))
            return false;
        return true;
    }
    case JSC::op_create_generator_frame_environment: {
        auto decoded = instruction->as<JSC::OpCreateGeneratorFrameEnvironment>();
        if (!emitSignedOperand(visitorContext, "operand.op_create_generator_frame_environment.dst", 49, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_create_generator_frame_environment.scope", 51, 0, static_cast<int64_t>(decoded.m_scope.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_create_generator_frame_environment.symbolTable", 57, 0, static_cast<int64_t>(decoded.m_symbolTable.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_create_generator_frame_environment.initialValue", 58, 0, static_cast<int64_t>(decoded.m_initialValue.offset())))
            return false;
        return true;
    }
    case JSC::op_get_parent_scope: {
        auto decoded = instruction->as<JSC::OpGetParentScope>();
        if (!emitSignedOperand(visitorContext, "operand.op_get_parent_scope.dst", 31, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_get_parent_scope.scope", 33, 0, static_cast<int64_t>(decoded.m_scope.offset())))
            return false;
        return true;
    }
    case JSC::op_create_rest: {
        auto decoded = instruction->as<JSC::OpCreateRest>();
        if (!emitSignedOperand(visitorContext, "operand.op_create_rest.dst", 26, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_create_rest.numParametersToSkip", 42, 7, static_cast<uint64_t>(decoded.m_numParametersToSkip)))
            return false;
        return true;
    }
    case JSC::op_yield: {
        auto decoded = instruction->as<JSC::OpYield>();
        if (!emitUnsignedOperand(visitorContext, "operand.op_yield.yieldPoint", 27, 21, static_cast<uint64_t>(decoded.m_yieldPoint)))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_yield.argument", 25, 0, static_cast<int64_t>(decoded.m_argument.offset())))
            return false;
        return true;
    }
    case JSC::op_get_scope: {
        auto decoded = instruction->as<JSC::OpGetScope>();
        if (!emitSignedOperand(visitorContext, "operand.op_get_scope.dst", 24, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        return true;
    }
    case JSC::op_create_direct_arguments: {
        auto decoded = instruction->as<JSC::OpCreateDirectArguments>();
        if (!emitSignedOperand(visitorContext, "operand.op_create_direct_arguments.dst", 38, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        return true;
    }
    case JSC::op_create_scoped_arguments: {
        auto decoded = instruction->as<JSC::OpCreateScopedArguments>();
        if (!emitSignedOperand(visitorContext, "operand.op_create_scoped_arguments.dst", 38, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_create_scoped_arguments.scope", 40, 0, static_cast<int64_t>(decoded.m_scope.offset())))
            return false;
        return true;
    }
    case JSC::op_create_cloned_arguments: {
        auto decoded = instruction->as<JSC::OpCreateClonedArguments>();
        if (!emitSignedOperand(visitorContext, "operand.op_create_cloned_arguments.dst", 38, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        return true;
    }
    case JSC::op_new_promise: {
        auto decoded = instruction->as<JSC::OpNewPromise>();
        if (!emitSignedOperand(visitorContext, "operand.op_new_promise.dst", 26, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        return true;
    }
    case JSC::op_new_generator: {
        auto decoded = instruction->as<JSC::OpNewGenerator>();
        if (!emitSignedOperand(visitorContext, "operand.op_new_generator.dst", 28, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        return true;
    }
    case JSC::op_new_async_function_generator: {
        auto decoded = instruction->as<JSC::OpNewAsyncFunctionGenerator>();
        if (!emitSignedOperand(visitorContext, "operand.op_new_async_function_generator.dst", 43, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        return true;
    }
    case JSC::op_argument_count: {
        auto decoded = instruction->as<JSC::OpArgumentCount>();
        if (!emitSignedOperand(visitorContext, "operand.op_argument_count.dst", 29, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        return true;
    }
    case JSC::op_check_tdz: {
        auto decoded = instruction->as<JSC::OpCheckTdz>();
        if (!emitSignedOperand(visitorContext, "operand.op_check_tdz.targetVirtualRegister", 42, 0, static_cast<int64_t>(decoded.m_targetVirtualRegister.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_check_tdz.identifier", 31, 4, static_cast<int64_t>(decoded.m_identifier.offset())))
            return false;
        return true;
    }
