    case JSC::op_instanceof: {
        auto decoded = instruction->as<JSC::OpInstanceof>();
        if (!emitSignedOperand(visitorContext, "operand.op_instanceof.dst", 25, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_instanceof.value", 27, 0, static_cast<int64_t>(decoded.m_value.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_instanceof.constructor", 33, 0, static_cast<int64_t>(decoded.m_constructor.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_instanceof.hasInstanceOrPrototype", 44, 2, static_cast<int64_t>(decoded.m_hasInstanceOrPrototype.offset())))
            return false;
        return true;
    }
    case JSC::op_set_private_brand: {
        auto decoded = instruction->as<JSC::OpSetPrivateBrand>();
        if (!emitSignedOperand(visitorContext, "operand.op_set_private_brand.base", 33, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_set_private_brand.brand", 34, 0, static_cast<int64_t>(decoded.m_brand.offset())))
            return false;
        return true;
    }
    case JSC::op_check_private_brand: {
        auto decoded = instruction->as<JSC::OpCheckPrivateBrand>();
        if (!emitSignedOperand(visitorContext, "operand.op_check_private_brand.base", 35, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_check_private_brand.brand", 36, 0, static_cast<int64_t>(decoded.m_brand.offset())))
            return false;
        return true;
    }
    case JSC::op_put_by_id: {
        auto decoded = instruction->as<JSC::OpPutById>();
        if (!emitSignedOperand(visitorContext, "operand.op_put_by_id.base", 25, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_put_by_id.property", 29, 9, static_cast<uint64_t>(decoded.m_property)))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_put_by_id.value", 26, 0, static_cast<int64_t>(decoded.m_value.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_put_by_id.flags", 26, 22, static_cast<uint64_t>(decoded.m_flags.isDirect()) | (static_cast<uint64_t>(decoded.m_flags.ecmaMode().value()) << 1)))
            return false;
        return true;
    }
    case JSC::op_new_array_with_size: {
        auto decoded = instruction->as<JSC::OpNewArrayWithSize>();
        if (!emitSignedOperand(visitorContext, "operand.op_new_array_with_size.dst", 34, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_new_array_with_size.length", 37, 0, static_cast<int64_t>(decoded.m_length.offset())))
            return false;
        return true;
    }
    case JSC::op_new_array_buffer: {
        auto decoded = instruction->as<JSC::OpNewArrayBuffer>();
        if (!emitSignedOperand(visitorContext, "operand.op_new_array_buffer.dst", 31, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_new_array_buffer.immutableButterfly", 46, 0, static_cast<int64_t>(decoded.m_immutableButterfly.offset())))
            return false;
        return true;
    }
    case JSC::op_get_by_id: {
        auto decoded = instruction->as<JSC::OpGetById>();
        if (!emitSignedOperand(visitorContext, "operand.op_get_by_id.dst", 24, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_get_by_id.base", 25, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_get_by_id.property", 29, 9, static_cast<uint64_t>(decoded.m_property)))
            return false;
        return true;
    }
    case JSC::op_get_length: {
        auto decoded = instruction->as<JSC::OpGetLength>();
        if (!emitSignedOperand(visitorContext, "operand.op_get_length.dst", 25, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_get_length.base", 26, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        return true;
    }
    case JSC::op_new_array_with_species: {
        auto decoded = instruction->as<JSC::OpNewArrayWithSpecies>();
        if (!emitSignedOperand(visitorContext, "operand.op_new_array_with_species.dst", 37, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_new_array_with_species.length", 40, 0, static_cast<int64_t>(decoded.m_length.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_new_array_with_species.array", 39, 0, static_cast<int64_t>(decoded.m_array.offset())))
            return false;
        return true;
    }
    case JSC::op_new_object: {
        auto decoded = instruction->as<JSC::OpNewObject>();
        if (!emitSignedOperand(visitorContext, "operand.op_new_object.dst", 25, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        return true;
    }
    case JSC::op_new_array: {
        auto decoded = instruction->as<JSC::OpNewArray>();
        if (!emitSignedOperand(visitorContext, "operand.op_new_array.dst", 24, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_new_array.argv", 25, 3, static_cast<int64_t>(decoded.m_argv.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_new_array.argc", 25, 6, static_cast<uint64_t>(decoded.m_argc)))
            return false;
        return true;
    }
    case JSC::op_put_private_name: {
        auto decoded = instruction->as<JSC::OpPutPrivateName>();
        if (!emitSignedOperand(visitorContext, "operand.op_put_private_name.base", 32, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_put_private_name.property", 36, 0, static_cast<int64_t>(decoded.m_property.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_put_private_name.value", 33, 0, static_cast<int64_t>(decoded.m_value.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_put_private_name.putKind", 35, 22, decoded.m_putKind.value()))
            return false;
        return true;
    }
    case JSC::op_get_private_name: {
        auto decoded = instruction->as<JSC::OpGetPrivateName>();
        if (!emitSignedOperand(visitorContext, "operand.op_get_private_name.dst", 31, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_get_private_name.base", 32, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_get_private_name.property", 36, 0, static_cast<int64_t>(decoded.m_property.offset())))
            return false;
        return true;
    }
    case JSC::op_get_by_val_with_this: {
        auto decoded = instruction->as<JSC::OpGetByValWithThis>();
        if (!emitSignedOperand(visitorContext, "operand.op_get_by_val_with_this.dst", 35, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_get_by_val_with_this.base", 36, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_get_by_val_with_this.thisValue", 41, 0, static_cast<int64_t>(decoded.m_thisValue.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_get_by_val_with_this.property", 40, 0, static_cast<int64_t>(decoded.m_property.offset())))
            return false;
        return true;
    }
    case JSC::op_get_by_val: {
        auto decoded = instruction->as<JSC::OpGetByVal>();
        if (!emitSignedOperand(visitorContext, "operand.op_get_by_val.dst", 25, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_get_by_val.base", 26, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_get_by_val.property", 30, 0, static_cast<int64_t>(decoded.m_property.offset())))
            return false;
        return true;
    }
    case JSC::op_put_by_val: {
        auto decoded = instruction->as<JSC::OpPutByVal>();
        if (!emitSignedOperand(visitorContext, "operand.op_put_by_val.base", 26, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_put_by_val.property", 30, 0, static_cast<int64_t>(decoded.m_property.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_put_by_val.value", 27, 0, static_cast<int64_t>(decoded.m_value.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_put_by_val.ecmaMode", 30, 22, decoded.m_ecmaMode.value()))
            return false;
        return true;
    }
    case JSC::op_put_by_val_direct: {
        auto decoded = instruction->as<JSC::OpPutByValDirect>();
        if (!emitSignedOperand(visitorContext, "operand.op_put_by_val_direct.base", 33, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_put_by_val_direct.property", 37, 0, static_cast<int64_t>(decoded.m_property.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_put_by_val_direct.value", 34, 0, static_cast<int64_t>(decoded.m_value.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_put_by_val_direct.ecmaMode", 37, 22, decoded.m_ecmaMode.value()))
            return false;
        return true;
    }
    case JSC::op_in_by_val: {
        auto decoded = instruction->as<JSC::OpInByVal>();
        if (!emitSignedOperand(visitorContext, "operand.op_in_by_val.dst", 24, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_in_by_val.base", 25, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_in_by_val.property", 29, 0, static_cast<int64_t>(decoded.m_property.offset())))
            return false;
        return true;
    }
    case JSC::op_enumerator_next: {
        auto decoded = instruction->as<JSC::OpEnumeratorNext>();
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_next.propertyName", 39, 1, static_cast<int64_t>(decoded.m_propertyName.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_next.mode", 31, 2, static_cast<int64_t>(decoded.m_mode.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_next.index", 32, 2, static_cast<int64_t>(decoded.m_index.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_next.base", 31, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_next.enumerator", 37, 0, static_cast<int64_t>(decoded.m_enumerator.offset())))
            return false;
        return true;
    }
    case JSC::op_enumerator_in_by_val: {
        auto decoded = instruction->as<JSC::OpEnumeratorInByVal>();
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_in_by_val.dst", 35, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_in_by_val.base", 36, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_in_by_val.mode", 36, 0, static_cast<int64_t>(decoded.m_mode.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_in_by_val.propertyName", 44, 0, static_cast<int64_t>(decoded.m_propertyName.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_in_by_val.index", 37, 0, static_cast<int64_t>(decoded.m_index.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_in_by_val.enumerator", 42, 0, static_cast<int64_t>(decoded.m_enumerator.offset())))
            return false;
        return true;
    }
    case JSC::op_enumerator_has_own_property: {
        auto decoded = instruction->as<JSC::OpEnumeratorHasOwnProperty>();
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_has_own_property.dst", 42, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_has_own_property.base", 43, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_has_own_property.mode", 43, 0, static_cast<int64_t>(decoded.m_mode.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_has_own_property.propertyName", 51, 0, static_cast<int64_t>(decoded.m_propertyName.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_has_own_property.index", 44, 0, static_cast<int64_t>(decoded.m_index.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_has_own_property.enumerator", 49, 0, static_cast<int64_t>(decoded.m_enumerator.offset())))
            return false;
        return true;
    }
    case JSC::op_enumerator_put_by_val: {
        auto decoded = instruction->as<JSC::OpEnumeratorPutByVal>();
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_put_by_val.base", 37, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_put_by_val.mode", 37, 0, static_cast<int64_t>(decoded.m_mode.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_put_by_val.propertyName", 45, 0, static_cast<int64_t>(decoded.m_propertyName.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_put_by_val.index", 38, 0, static_cast<int64_t>(decoded.m_index.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_put_by_val.enumerator", 43, 0, static_cast<int64_t>(decoded.m_enumerator.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_put_by_val.value", 38, 0, static_cast<int64_t>(decoded.m_value.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_enumerator_put_by_val.ecmaMode", 41, 22, decoded.m_ecmaMode.value()))
            return false;
        return true;
    }
    case JSC::op_enumerator_get_by_val: {
        auto decoded = instruction->as<JSC::OpEnumeratorGetByVal>();
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_get_by_val.dst", 36, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_get_by_val.base", 37, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_get_by_val.mode", 37, 0, static_cast<int64_t>(decoded.m_mode.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_get_by_val.propertyName", 45, 0, static_cast<int64_t>(decoded.m_propertyName.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_get_by_val.index", 38, 0, static_cast<int64_t>(decoded.m_index.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_enumerator_get_by_val.enumerator", 43, 0, static_cast<int64_t>(decoded.m_enumerator.offset())))
            return false;
        return true;
    }
    case JSC::op_get_by_id_direct: {
        auto decoded = instruction->as<JSC::OpGetByIdDirect>();
        if (!emitSignedOperand(visitorContext, "operand.op_get_by_id_direct.dst", 31, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_get_by_id_direct.base", 32, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_get_by_id_direct.property", 36, 9, static_cast<uint64_t>(decoded.m_property)))
            return false;
        return true;
    }
    case JSC::op_get_prototype_of: {
        auto decoded = instruction->as<JSC::OpGetPrototypeOf>();
        if (!emitSignedOperand(visitorContext, "operand.op_get_prototype_of.dst", 31, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_get_prototype_of.value", 33, 0, static_cast<int64_t>(decoded.m_value.offset())))
            return false;
        return true;
    }
    case JSC::op_get_internal_field: {
        auto decoded = instruction->as<JSC::OpGetInternalField>();
        if (!emitSignedOperand(visitorContext, "operand.op_get_internal_field.dst", 33, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_get_internal_field.base", 34, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_get_internal_field.index", 35, 14, static_cast<uint64_t>(decoded.m_index)))
            return false;
        return true;
    }
    case JSC::op_get_by_id_with_this: {
        auto decoded = instruction->as<JSC::OpGetByIdWithThis>();
        if (!emitSignedOperand(visitorContext, "operand.op_get_by_id_with_this.dst", 34, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_get_by_id_with_this.base", 35, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_get_by_id_with_this.thisValue", 40, 0, static_cast<int64_t>(decoded.m_thisValue.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_get_by_id_with_this.property", 39, 9, static_cast<uint64_t>(decoded.m_property)))
            return false;
        return true;
    }
    case JSC::op_to_object: {
        auto decoded = instruction->as<JSC::OpToObject>();
        if (!emitSignedOperand(visitorContext, "operand.op_to_object.dst", 24, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_to_object.operand", 28, 0, static_cast<int64_t>(decoded.m_operand.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_to_object.message", 28, 9, static_cast<uint64_t>(decoded.m_message)))
            return false;
        return true;
    }
    case JSC::op_in_by_id: {
        auto decoded = instruction->as<JSC::OpInById>();
        if (!emitSignedOperand(visitorContext, "operand.op_in_by_id.dst", 23, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_in_by_id.base", 24, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_in_by_id.property", 28, 9, static_cast<uint64_t>(decoded.m_property)))
            return false;
        return true;
    }
    case JSC::op_has_private_name: {
        auto decoded = instruction->as<JSC::OpHasPrivateName>();
        if (!emitSignedOperand(visitorContext, "operand.op_has_private_name.dst", 31, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_has_private_name.base", 32, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_has_private_name.property", 36, 0, static_cast<int64_t>(decoded.m_property.offset())))
            return false;
        return true;
    }
    case JSC::op_has_private_brand: {
        auto decoded = instruction->as<JSC::OpHasPrivateBrand>();
        if (!emitSignedOperand(visitorContext, "operand.op_has_private_brand.dst", 32, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_has_private_brand.base", 33, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_has_private_brand.brand", 34, 0, static_cast<int64_t>(decoded.m_brand.offset())))
            return false;
        return true;
    }
    case JSC::op_put_by_id_with_this: {
        auto decoded = instruction->as<JSC::OpPutByIdWithThis>();
        if (!emitSignedOperand(visitorContext, "operand.op_put_by_id_with_this.base", 35, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_put_by_id_with_this.thisValue", 40, 0, static_cast<int64_t>(decoded.m_thisValue.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_put_by_id_with_this.property", 39, 9, static_cast<uint64_t>(decoded.m_property)))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_put_by_id_with_this.value", 36, 0, static_cast<int64_t>(decoded.m_value.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_put_by_id_with_this.ecmaMode", 39, 22, decoded.m_ecmaMode.value()))
            return false;
        return true;
    }
    case JSC::op_del_by_id: {
        auto decoded = instruction->as<JSC::OpDelById>();
        if (!emitSignedOperand(visitorContext, "operand.op_del_by_id.dst", 24, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_del_by_id.base", 25, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_del_by_id.property", 29, 9, static_cast<uint64_t>(decoded.m_property)))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_del_by_id.ecmaMode", 29, 22, decoded.m_ecmaMode.value()))
            return false;
        return true;
    }
    case JSC::op_put_by_val_with_this: {
        auto decoded = instruction->as<JSC::OpPutByValWithThis>();
        if (!emitSignedOperand(visitorContext, "operand.op_put_by_val_with_this.base", 36, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_put_by_val_with_this.thisValue", 41, 0, static_cast<int64_t>(decoded.m_thisValue.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_put_by_val_with_this.property", 40, 0, static_cast<int64_t>(decoded.m_property.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_put_by_val_with_this.value", 37, 0, static_cast<int64_t>(decoded.m_value.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_put_by_val_with_this.ecmaMode", 40, 22, decoded.m_ecmaMode.value()))
            return false;
        return true;
    }
    case JSC::op_del_by_val: {
        auto decoded = instruction->as<JSC::OpDelByVal>();
        if (!emitSignedOperand(visitorContext, "operand.op_del_by_val.dst", 25, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_del_by_val.base", 26, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_del_by_val.property", 30, 0, static_cast<int64_t>(decoded.m_property.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_del_by_val.ecmaMode", 30, 22, decoded.m_ecmaMode.value()))
            return false;
        return true;
    }
    case JSC::op_put_getter_by_id: {
        auto decoded = instruction->as<JSC::OpPutGetterById>();
        if (!emitSignedOperand(visitorContext, "operand.op_put_getter_by_id.base", 32, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_put_getter_by_id.property", 36, 9, static_cast<uint64_t>(decoded.m_property)))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_put_getter_by_id.attributes", 38, 16, static_cast<uint64_t>(decoded.m_attributes)))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_put_getter_by_id.accessor", 36, 0, static_cast<int64_t>(decoded.m_accessor.offset())))
            return false;
        return true;
    }
    case JSC::op_put_setter_by_id: {
        auto decoded = instruction->as<JSC::OpPutSetterById>();
        if (!emitSignedOperand(visitorContext, "operand.op_put_setter_by_id.base", 32, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_put_setter_by_id.property", 36, 9, static_cast<uint64_t>(decoded.m_property)))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_put_setter_by_id.attributes", 38, 16, static_cast<uint64_t>(decoded.m_attributes)))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_put_setter_by_id.accessor", 36, 0, static_cast<int64_t>(decoded.m_accessor.offset())))
            return false;
        return true;
    }
    case JSC::op_put_getter_setter_by_id: {
        auto decoded = instruction->as<JSC::OpPutGetterSetterById>();
        if (!emitSignedOperand(visitorContext, "operand.op_put_getter_setter_by_id.base", 39, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_put_getter_setter_by_id.property", 43, 9, static_cast<uint64_t>(decoded.m_property)))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_put_getter_setter_by_id.attributes", 45, 16, static_cast<uint64_t>(decoded.m_attributes)))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_put_getter_setter_by_id.getter", 41, 0, static_cast<int64_t>(decoded.m_getter.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_put_getter_setter_by_id.setter", 41, 0, static_cast<int64_t>(decoded.m_setter.offset())))
            return false;
        return true;
    }
    case JSC::op_put_getter_by_val: {
        auto decoded = instruction->as<JSC::OpPutGetterByVal>();
        if (!emitSignedOperand(visitorContext, "operand.op_put_getter_by_val.base", 33, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_put_getter_by_val.property", 37, 0, static_cast<int64_t>(decoded.m_property.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_put_getter_by_val.attributes", 39, 16, static_cast<uint64_t>(decoded.m_attributes)))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_put_getter_by_val.accessor", 37, 0, static_cast<int64_t>(decoded.m_accessor.offset())))
            return false;
        return true;
    }
    case JSC::op_put_setter_by_val: {
        auto decoded = instruction->as<JSC::OpPutSetterByVal>();
        if (!emitSignedOperand(visitorContext, "operand.op_put_setter_by_val.base", 33, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_put_setter_by_val.property", 37, 0, static_cast<int64_t>(decoded.m_property.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_put_setter_by_val.attributes", 39, 16, static_cast<uint64_t>(decoded.m_attributes)))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_put_setter_by_val.accessor", 37, 0, static_cast<int64_t>(decoded.m_accessor.offset())))
            return false;
        return true;
    }
    case JSC::op_define_data_property: {
        auto decoded = instruction->as<JSC::OpDefineDataProperty>();
        if (!emitSignedOperand(visitorContext, "operand.op_define_data_property.base", 36, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_define_data_property.property", 40, 0, static_cast<int64_t>(decoded.m_property.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_define_data_property.value", 37, 0, static_cast<int64_t>(decoded.m_value.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_define_data_property.attributes", 42, 0, static_cast<int64_t>(decoded.m_attributes.offset())))
            return false;
        return true;
    }
    case JSC::op_define_accessor_property: {
        auto decoded = instruction->as<JSC::OpDefineAccessorProperty>();
        if (!emitSignedOperand(visitorContext, "operand.op_define_accessor_property.base", 40, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_define_accessor_property.property", 44, 0, static_cast<int64_t>(decoded.m_property.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_define_accessor_property.getter", 42, 0, static_cast<int64_t>(decoded.m_getter.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_define_accessor_property.setter", 42, 0, static_cast<int64_t>(decoded.m_setter.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_define_accessor_property.attributes", 46, 0, static_cast<int64_t>(decoded.m_attributes.offset())))
            return false;
        return true;
    }
    case JSC::op_strcat: {
        auto decoded = instruction->as<JSC::OpStrcat>();
        if (!emitSignedOperand(visitorContext, "operand.op_strcat.dst", 21, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_strcat.src", 21, 3, static_cast<int64_t>(decoded.m_src.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_strcat.count", 23, 24, static_cast<int64_t>(decoded.m_count)))
            return false;
        return true;
    }
    case JSC::op_to_property_key: {
        auto decoded = instruction->as<JSC::OpToPropertyKey>();
        if (!emitSignedOperand(visitorContext, "operand.op_to_property_key.dst", 30, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_to_property_key.src", 30, 0, static_cast<int64_t>(decoded.m_src.offset())))
            return false;
        return true;
    }
    case JSC::op_to_property_key_or_number: {
        auto decoded = instruction->as<JSC::OpToPropertyKeyOrNumber>();
        if (!emitSignedOperand(visitorContext, "operand.op_to_property_key_or_number.dst", 40, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_to_property_key_or_number.src", 40, 0, static_cast<int64_t>(decoded.m_src.offset())))
            return false;
        return true;
    }
    case JSC::op_get_property_enumerator: {
        auto decoded = instruction->as<JSC::OpGetPropertyEnumerator>();
        if (!emitSignedOperand(visitorContext, "operand.op_get_property_enumerator.dst", 38, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_get_property_enumerator.base", 39, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        return true;
    }
    case JSC::op_put_internal_field: {
        auto decoded = instruction->as<JSC::OpPutInternalField>();
        if (!emitSignedOperand(visitorContext, "operand.op_put_internal_field.base", 34, 0, static_cast<int64_t>(decoded.m_base.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_put_internal_field.index", 35, 14, static_cast<uint64_t>(decoded.m_index)))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_put_internal_field.value", 35, 0, static_cast<int64_t>(decoded.m_value.offset())))
            return false;
        return true;
    }
    case JSC::op_new_array_with_spread: {
        auto decoded = instruction->as<JSC::OpNewArrayWithSpread>();
        if (!emitSignedOperand(visitorContext, "operand.op_new_array_with_spread.dst", 36, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_new_array_with_spread.argv", 37, 3, static_cast<int64_t>(decoded.m_argv.offset())))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_new_array_with_spread.argc", 37, 6, static_cast<uint64_t>(decoded.m_argc)))
            return false;
        if (!emitUnsignedOperand(visitorContext, "operand.op_new_array_with_spread.bitVector", 42, 12, static_cast<uint64_t>(decoded.m_bitVector)))
            return false;
        return true;
    }
    case JSC::op_spread: {
        auto decoded = instruction->as<JSC::OpSpread>();
        if (!emitSignedOperand(visitorContext, "operand.op_spread.dst", 21, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_spread.argument", 26, 0, static_cast<int64_t>(decoded.m_argument.offset())))
            return false;
        return true;
    }
    case JSC::op_new_reg_exp: {
        auto decoded = instruction->as<JSC::OpNewRegExp>();
        if (!emitSignedOperand(visitorContext, "operand.op_new_reg_exp.dst", 26, 1, static_cast<int64_t>(decoded.m_dst.offset())))
            return false;
        if (!emitSignedOperand(visitorContext, "operand.op_new_reg_exp.regexp", 29, 4, static_cast<int64_t>(decoded.m_regexp.offset())))
            return false;
        return true;
    }
