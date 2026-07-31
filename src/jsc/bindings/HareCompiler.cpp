#include "root.h"

#include "HareApplication.h"
#include "HareCompiler.h"
#include "ZigSourceProvider.h"
#include "helpers.h"

#include <bit>
#include <cstdio>
#include <limits>
#include <memory>
#include <JavaScriptCore/Instruction.h>
#include <JavaScriptCore/LinkTimeConstant.h>

namespace JSC {
struct OpNop;
class UnlinkedCodeBlockGenerator;

// The packaged JSC SDK omits BytecodeGenerator.h, but its generated typed
// accessors instantiate BoundLabel::target() through this pinned traits shape.
struct JSGeneratorTraits {
    using OpcodeTraits = JSOpcodeTraits;
    using OpcodeID = ::JSC::OpcodeID;
    using OpNop = ::JSC::OpNop;
    using CodeBlock = std::unique_ptr<UnlinkedCodeBlockGenerator>;
    using InstructionType = JSInstruction;
    static constexpr OpcodeID opcodeForDisablingOptimizations = op_debug;
};
}

#include <JavaScriptCore/CodeCache.h>
#include <JavaScriptCore/BytecodeStructs.h>
#include <JavaScriptCore/DeferGC.h>
#include <JavaScriptCore/InstructionStream.h>
#include <JavaScriptCore/JSCJSValueInlines.h>
#include <JavaScriptCore/JSCellButterfly.h>
#include <JavaScriptCore/JSString.h>
#include <JavaScriptCore/ParserError.h>
#include <JavaScriptCore/RegExp.h>
#include <JavaScriptCore/SourceCodeKey.h>
#include <JavaScriptCore/Strong.h>
#include <JavaScriptCore/StrongInlines.h>
#include <JavaScriptCore/SymbolTable.h>
#include <JavaScriptCore/TopExceptionScope.h>
#include <JavaScriptCore/UnlinkedCodeBlock.h>
#include <JavaScriptCore/UnlinkedEvalCodeBlock.h>
#include <JavaScriptCore/UnlinkedFunctionCodeBlock.h>
#include <JavaScriptCore/UnlinkedFunctionExecutable.h>
#include <JavaScriptCore/UnlinkedModuleProgramCodeBlock.h>
#include <JavaScriptCore/UnlinkedProgramCodeBlock.h>
#include <new>
#include <wtf/Vector.h>

extern "C" uint32_t Bun__Hare__visitorBeginFunction(
    void*, uint32_t, uint32_t, uint32_t, uint32_t, uint32_t, uint32_t,
    uint32_t, uint32_t, uint32_t, uint32_t, uint32_t, uint32_t, uint32_t,
    int32_t, int32_t, int32_t, int32_t, int32_t, uint32_t);
extern "C" uint32_t Bun__Hare__visitorInstruction(
    void*, uint32_t, uint32_t, uint32_t, uint32_t, uint32_t);
extern "C" uint32_t Bun__Hare__visitorConstantScalar(
    void*, uint32_t, uint32_t, uint32_t, uint64_t);
extern "C" uint32_t Bun__Hare__visitorConstantText(
    void*, uint32_t, uint32_t, uint32_t, const void*, size_t, uint32_t);
extern "C" uint32_t Bun__Hare__visitorBeginArrayConstant(
    void*, uint32_t, uint32_t, uint32_t, uint32_t);
extern "C" uint32_t Bun__Hare__visitorArrayConstantScalar(
    void*, uint32_t, uint32_t, uint64_t);
extern "C" uint32_t Bun__Hare__visitorArrayConstantText(
    void*, uint32_t, const void*, size_t, uint32_t);
extern "C" uint32_t Bun__Hare__visitorEndArrayConstant(void*);
extern "C" uint32_t Bun__Hare__visitorRegExpConstant(
    void*, uint32_t, uint32_t, const void*, size_t, uint32_t, uint32_t);
extern "C" uint32_t Bun__Hare__visitorIdentifier(
    void*, uint32_t, const void*, size_t, uint32_t);
extern "C" uint32_t Bun__Hare__visitorSimpleSwitchTable(
    void*, uint32_t, int32_t, int32_t, uint32_t, const int32_t*, size_t);
extern "C" uint32_t Bun__Hare__visitorBeginStringSwitchTable(
    void*, uint32_t, uint32_t, uint32_t, int32_t, uint32_t);
extern "C" uint32_t Bun__Hare__visitorStringSwitchEntry(
    void*, uint32_t, const void*, size_t, uint32_t, int32_t, uint32_t);
extern "C" uint32_t Bun__Hare__visitorExceptionHandler(
    void*, uint32_t, uint32_t, uint32_t, uint32_t, uint32_t);
extern "C" uint32_t Bun__Hare__visitorOperand(
    void*, const uint8_t*, size_t, uint32_t, uint32_t, int64_t, uint64_t);
extern "C" uint32_t Bun__Hare__visitorParserError(
    void*, uint32_t, uint32_t, int32_t, uint32_t, const void*, size_t, uint32_t);
extern "C" uint64_t Bun__Hare__nativeProbe(uint64_t);
extern "C" [[noreturn]] void Bun__outOfMemory();

namespace Bun::Hare {

static constexpr uint32_t noParent = UINT32_MAX;
static constexpr uint64_t nativeProbeExpected = 0x484152455f573101ULL;

enum class Relation : uint32_t {
    Root = 0,
    Declaration = 1,
    Expression = 2,
    FunctionConstructorSpecialization = 3,
};

enum class Specialization : uint32_t {
    Program = 0,
    Module = 1,
    Eval = 2,
    Call = 3,
    Construct = 4,
    FunctionConstructorCall = 5,
    FunctionConstructorConstruct = 6,
};

struct FunctionRecord {
    FunctionRecord(
        JSC::VM& vm, JSC::UnlinkedCodeBlock* codeBlock,
        const JSC::SourceCode& source, uint32_t parent, Relation relation,
        uint32_t relationIndex, Specialization specialization)
        : block(vm, codeBlock)
        , source(source)
        , parent(parent)
        , relation(relation)
        , relationIndex(relationIndex)
        , specialization(specialization)
    {
    }

    JSC::Strong<JSC::UnlinkedCodeBlock> block;
    JSC::SourceCode source;
    uint32_t parent;
    Relation relation;
    uint32_t relationIndex;
    Specialization specialization;
};

using FunctionRecords = Vector<std::unique_ptr<FunctionRecord>>;

static ImportResult result(ImportStatus status, uint32_t detail = 0)
{
    return { status, detail };
}

static ImportResult emitParserError(void* visitorContext, const JSC::ParserError& error)
{
    if (error.type() == JSC::ParserError::OutOfMemory)
        Bun__outOfMemory();

    auto location = error.token().location();
    uint32_t column = location.startOffset >= location.lineStartOffset
        ? location.startOffset - location.lineStartOffset
        : 0;
    const auto& message = error.message();
    uint32_t accepted;
    if (message.is8Bit()) {
        auto span = message.span8();
        accepted = Bun__Hare__visitorParserError(
            visitorContext, static_cast<uint32_t>(error.type()),
            static_cast<uint32_t>(error.syntaxErrorType()), error.line(), column,
            span.data(), span.size(), 1);
    } else {
        auto span = message.span16();
        accepted = Bun__Hare__visitorParserError(
            visitorContext, static_cast<uint32_t>(error.type()),
            static_cast<uint32_t>(error.syntaxErrorType()), error.line(), column,
            span.data(), span.size(), 0);
    }
    return accepted ? result(ImportStatus::ParserError)
                    : result(ImportStatus::VisitorRejected, 1);
}

static bool emitSignedOperand(
    void* visitorContext, const char* manifestId, size_t manifestIdLength,
    uint32_t role, int64_t value)
{
    return Bun__Hare__visitorOperand(
        visitorContext, reinterpret_cast<const uint8_t*>(manifestId),
        manifestIdLength, role, 0, value, 0);
}

static bool emitUnsignedOperand(
    void* visitorContext, const char* manifestId, size_t manifestIdLength,
    uint32_t role, uint64_t value)
{
    return Bun__Hare__visitorOperand(
        visitorContext, reinterpret_cast<const uint8_t*>(manifestId),
        manifestIdLength, role, 1, 0, value);
}

static bool emitBooleanOperand(
    void* visitorContext, const char* manifestId, size_t manifestIdLength,
    uint32_t role, bool value)
{
    return Bun__Hare__visitorOperand(
        visitorContext, reinterpret_cast<const uint8_t*>(manifestId),
        manifestIdLength, role, 2, 0, value ? 1 : 0);
}

static bool visitInstructionOperands(
    JSC::UnlinkedCodeBlock& block,
    const JSC::JSInstructionStream::Ref& instructionRef,
    void* visitorContext)
{
    const JSC::JSInstruction* instruction = instructionRef.ptr();
    switch (instruction->opcodeID()) {
#include "../../../generated/hare/control/visitor.inc"
#include "../../../generated/hare/numeric/visitor.inc"
#include "../../../generated/hare/object/visitor.inc"
#include "../../../generated/hare/function/visitor.inc"
#include "../../../generated/hare/exception/visitor.inc"
#include "../../../generated/hare/module/visitor.inc"
#include "../../../generated/hare/cache/visitor.inc"
    default:
        return false;
    }
}

static bool emitCopiedText(
    void* visitorContext, uint32_t index, uint32_t kind,
    JSC::SourceCodeRepresentation sourceRepresentation, const WTF::String& text)
{
    if (text.is8Bit()) {
        auto span = text.span8();
        return Bun__Hare__visitorConstantText(
            visitorContext, index, kind,
            static_cast<uint32_t>(sourceRepresentation), span.data(), span.size(), 1);
    }
    auto span = text.span16();
    return Bun__Hare__visitorConstantText(
        visitorContext, index, kind,
        static_cast<uint32_t>(sourceRepresentation), span.data(), span.size(), 0);
}

static bool emitArrayConstantElement(
    void* visitorContext, uint32_t index, JSC::JSValue value)
{
    uint32_t kind;
    uint64_t payload = 0;
    if (value.isEmpty())
        kind = 0;
    else if (value.isUndefined())
        kind = 1;
    else if (value.isNull())
        kind = 2;
    else if (value.isBoolean()) {
        kind = 3;
        payload = value.asBoolean();
    } else if (value.isInt32()) {
        kind = 4;
        payload = static_cast<uint64_t>(static_cast<int64_t>(value.asInt32()));
    } else if (value.isDouble()) {
        kind = 5;
        payload = std::bit_cast<uint64_t>(value.asNumber());
    } else if (value.isString()) {
        auto holder = JSC::asString(value)->tryGetValue();
        const WTF::String& string = holder.data;
        if (string.isNull())
            return false;
        if (string.is8Bit()) {
            auto span = string.span8();
            return Bun__Hare__visitorArrayConstantText(
                visitorContext, index, span.data(), span.size(), 1);
        }
        auto span = string.span16();
        return Bun__Hare__visitorArrayConstantText(
            visitorContext, index, span.data(), span.size(), 0);
    } else
        return false;
    return Bun__Hare__visitorArrayConstantScalar(
        visitorContext, index, kind, payload);
}

static bool emitArrayConstant(
    void* visitorContext, uint32_t index,
    JSC::SourceCodeRepresentation sourceRepresentation,
    JSC::JSCellButterfly& array)
{
    uint32_t length = array.length();
    if (!Bun__Hare__visitorBeginArrayConstant(
            visitorContext, index, static_cast<uint32_t>(sourceRepresentation),
            static_cast<uint32_t>(array.indexingTypeAndMisc()), length))
        return false;
    for (uint32_t elementIndex = 0; elementIndex < length; ++elementIndex) {
        if (!emitArrayConstantElement(
                visitorContext, elementIndex, array.get(elementIndex)))
            return false;
    }
    return Bun__Hare__visitorEndArrayConstant(visitorContext);
}

static bool emitLinkTimeConstant(
    void* visitorContext, uint32_t index,
    JSC::SourceCodeRepresentation sourceRepresentation, int32_t rawValue)
{
    if (rawValue < 0
        || static_cast<uint32_t>(rawValue) >= JSC::numberOfLinkTimeConstants)
        return false;

    switch (static_cast<JSC::LinkTimeConstant>(rawValue)) {
#define EMIT_LINK_TIME_CONSTANT(name, code)                                    \
    case JSC::LinkTimeConstant::name: {                                        \
        static constexpr char constantName[] = #name;                          \
        return Bun__Hare__visitorConstantText(                                 \
            visitorContext, index, 8, static_cast<uint32_t>(sourceRepresentation), \
            constantName, sizeof(constantName) - 1, 1);                        \
    }
        JSC_FOREACH_LINK_TIME_CONSTANTS(EMIT_LINK_TIME_CONSTANT)
#undef EMIT_LINK_TIME_CONSTANT
    }
    return false;
}

static bool emitRegExpConstant(
    void* visitorContext, uint32_t index,
    JSC::SourceCodeRepresentation sourceRepresentation, JSC::RegExp& regexp)
{
    const WTF::String& pattern = regexp.pattern();
    uint32_t flags = static_cast<uint32_t>(regexp.flags().toRaw());
    if (pattern.is8Bit()) {
        auto span = pattern.span8();
        return Bun__Hare__visitorRegExpConstant(
            visitorContext, index, static_cast<uint32_t>(sourceRepresentation),
            span.data(), span.size(), 1, flags);
    }
    auto span = pattern.span16();
    return Bun__Hare__visitorRegExpConstant(
        visitorContext, index, static_cast<uint32_t>(sourceRepresentation),
        span.data(), span.size(), 0, flags);
}

static bool visitConstantsAndIdentifiers(
    JSC::UnlinkedCodeBlock& block, void* visitorContext)
{
    uint32_t index = 0;
    for (const auto& barrier : block.constantRegisters()) {
        JSC::JSValue value = barrier.get();
        auto sourceRepresentation = block.constantSourceCodeRepresentation(index);
        uint32_t sourceRepresentationRaw = static_cast<uint32_t>(sourceRepresentation);
        if (sourceRepresentation == JSC::SourceCodeRepresentation::LinkTimeConstant) {
            if (!value.isInt32()
                || !emitLinkTimeConstant(
                    visitorContext, index, sourceRepresentation, value.asInt32()))
                return false;
            ++index;
            continue;
        }
        uint32_t kind;
        uint64_t payload = 0;
        if (value.isEmpty())
            kind = 0;
        else if (value.isUndefined())
            kind = 1;
        else if (value.isNull())
            kind = 2;
        else if (value.isBoolean()) {
            kind = 3;
            payload = value.asBoolean();
        } else if (value.isInt32()) {
            kind = 4;
            payload = static_cast<uint64_t>(static_cast<int64_t>(value.asInt32()));
        } else if (value.isDouble()) {
            kind = 5;
            payload = std::bit_cast<uint64_t>(value.asNumber());
        } else if (value.isString()) {
            auto holder = JSC::asString(value)->tryGetValue();
            const WTF::String& string = holder.data;
            if (string.isNull()
                || !emitCopiedText(
                    visitorContext, index, 6, sourceRepresentation, string))
                return false;
            ++index;
            continue;
        } else if (auto* array = dynamicDowncast<JSC::JSCellButterfly>(value)) {
            if (!emitArrayConstant(
                    visitorContext, index, sourceRepresentation, *array))
                return false;
            ++index;
            continue;
        } else if (auto* regexp = dynamicDowncast<JSC::RegExp>(value)) {
            if (!emitRegExpConstant(
                    visitorContext, index, sourceRepresentation, *regexp))
                return false;
            ++index;
            continue;
        } else {
            static constexpr char symbolTable[] = "SymbolTable";
            static constexpr char unsupportedCell[] = "UnsupportedCell";
            bool isSymbolTable = dynamicDowncast<JSC::SymbolTable>(value);
            const char* name = isSymbolTable
                ? symbolTable
                : unsupportedCell;
            size_t length = isSymbolTable
                ? sizeof(symbolTable) - 1
                : sizeof(unsupportedCell) - 1;
            if (!Bun__Hare__visitorConstantText(
                    visitorContext, index, 7, sourceRepresentationRaw,
                    name, length, 1))
                return false;
            ++index;
            continue;
        }
        if (!Bun__Hare__visitorConstantScalar(
                visitorContext, index, kind, sourceRepresentationRaw, payload))
            return false;
        ++index;
    }

    index = 0;
    for (const auto& identifier : block.identifiers()) {
        const WTF::String& string = identifier.string().string();
        uint32_t accepted;
        if (string.is8Bit()) {
            auto span = string.span8();
            accepted = Bun__Hare__visitorIdentifier(
                visitorContext, index, span.data(), span.size(), 1);
        } else {
            auto span = string.span16();
            accepted = Bun__Hare__visitorIdentifier(
                visitorContext, index, span.data(), span.size(), 0);
        }
        if (!accepted)
            return false;
        ++index;
    }
    return true;
}

static bool visitSwitchTables(JSC::UnlinkedCodeBlock& block, void* visitorContext)
{
    size_t simpleCount = block.numberOfUnlinkedSwitchJumpTables();
    size_t stringCount = block.numberOfUnlinkedStringSwitchJumpTables();
    if (simpleCount > UINT32_MAX || stringCount > UINT32_MAX)
        return false;

    for (uint32_t index = 0; index < simpleCount; ++index) {
        const auto& table = block.unlinkedSwitchJumpTable(index);
        auto branchOffsets = table.m_branchOffsets.span();
        if (!Bun__Hare__visitorSimpleSwitchTable(
                visitorContext, index, table.m_min, table.m_defaultOffset,
                table.isList(), branchOffsets.data(), branchOffsets.size()))
            return false;
    }

    for (uint32_t index = 0; index < stringCount; ++index) {
        const auto& table = block.unlinkedStringSwitchJumpTable(index);
        if (table.m_offsetTable.size() > UINT32_MAX
            || !Bun__Hare__visitorBeginStringSwitchTable(
                visitorContext, index, table.m_minLength, table.m_maxLength,
                table.m_defaultOffset,
                static_cast<uint32_t>(table.m_offsetTable.size())))
            return false;
        for (const auto& entry : table.m_offsetTable) {
            auto* key = entry.key.get();
            uint32_t accepted;
            if (key->is8Bit()) {
                auto span = key->span8();
                accepted = Bun__Hare__visitorStringSwitchEntry(
                    visitorContext, index, span.data(), span.size(), 1,
                    entry.value.m_branchOffset, entry.value.m_indexInTable);
            } else {
                auto span = key->span16();
                accepted = Bun__Hare__visitorStringSwitchEntry(
                    visitorContext, index, span.data(), span.size(), 0,
                    entry.value.m_branchOffset, entry.value.m_indexInTable);
            }
            if (!accepted)
                return false;
        }
    }
    return true;
}

static bool visitExceptionHandlers(JSC::UnlinkedCodeBlock& block, void* visitorContext)
{
    size_t count = block.numberOfExceptionHandlers();
    if (count > UINT32_MAX)
        return false;
    for (uint32_t index = 0; index < count; ++index) {
        const auto& handler = block.exceptionHandler(index);
        if (!Bun__Hare__visitorExceptionHandler(
                visitorContext, index, handler.start, handler.end,
                handler.target, handler.typeBits))
            return false;
    }
    return true;
}

static ImportResult materializeChildren(
    JSC::VM& vm, FunctionRecords& records, uint32_t parentId,
    void* visitorContext)
{
    auto* parentRecord = records[parentId].get();
    size_t declarationCount = parentRecord->block.get()->numberOfFunctionDecls();
    size_t expressionCount = parentRecord->block.get()->numberOfFunctionExprs();

    auto materialize = [&](bool declaration, uint32_t index) -> ImportResult {
        auto* parentBlock = parentRecord->block.get();
        auto* executable = declaration
            ? parentBlock->functionDecl(static_cast<int>(index))
            : parentBlock->functionExpr(static_cast<int>(index));
        auto source = executable->linkedSourceCode(parentRecord->source);
        auto materializeSpecialization = [&](
            JSC::CodeSpecializationKind codeKind,
            Specialization specialization) -> ImportResult {
            JSC::ParserError parserError;
            auto* child = executable->unlinkedCodeBlockFor(
                vm, source, codeKind, parentBlock->codeGenerationMode(), parserError,
                executable->parseMode());
            if (parserError.isValid())
                return emitParserError(visitorContext, parserError);
            if (!child)
                return result(ImportStatus::NullRoot, 1);

            uint32_t childId = static_cast<uint32_t>(records.size());
            records.append(std::make_unique<FunctionRecord>(
                vm, child, source, parentId,
                declaration ? Relation::Declaration : Relation::Expression, index,
                specialization));
            return materializeChildren(vm, records, childId, visitorContext);
        };

        auto importResult = materializeSpecialization(
            JSC::CodeSpecializationKind::CodeForCall, Specialization::Call);
        if (importResult.status != ImportStatus::Success)
            return importResult;
        if (executable->constructAbility() == JSC::ConstructAbility::CanConstruct)
            return materializeSpecialization(
                JSC::CodeSpecializationKind::CodeForConstruct, Specialization::Construct);
        return result(ImportStatus::Success);
    };

    for (uint32_t index = 0; index < declarationCount; ++index) {
        auto importResult = materialize(true, index);
        if (importResult.status != ImportStatus::Success)
            return importResult;
    }
    for (uint32_t index = 0; index < expressionCount; ++index) {
        auto importResult = materialize(false, index);
        if (importResult.status != ImportStatus::Success)
            return importResult;
    }
    return result(ImportStatus::Success);
}

static ImportResult visitRecords(JSC::VM& vm, const FunctionRecords& records, void* visitorContext)
{
    for (uint32_t functionId = 0; functionId < records.size(); ++functionId) {
        uint32_t parent;
        uint32_t relation;
        uint32_t relationIndex;
        uint32_t specialization;
        uint32_t parseMode;
        uint32_t scriptMode;
        uint32_t codeType;
        uint32_t lexicalFeatures;
        uint32_t codeFeatures;
        uint32_t numParameters;
        uint32_t numVars;
        uint32_t numCalleeLocals;
        int32_t thisRegister;
        int32_t scopeRegister;
        int32_t callFrameCalleeRegister;
        int32_t callFrameThisArgumentRegister;
        int32_t callFrameFirstArgumentRegister;
        uint32_t instructionBytes;
        {
            auto* record = records[functionId].get();
            auto* block = record->block.get();
            parent = record->parent;
            relation = static_cast<uint32_t>(record->relation);
            relationIndex = record->relationIndex;
            specialization = static_cast<uint32_t>(record->specialization);
            parseMode = static_cast<uint32_t>(block->parseMode());
            scriptMode = static_cast<uint32_t>(block->scriptMode());
            codeType = static_cast<uint32_t>(block->codeType());
            lexicalFeatures = static_cast<uint32_t>(block->lexicallyScopedFeatures());
            codeFeatures = static_cast<uint32_t>(block->codeFeatures());
            numParameters = block->numParameters();
            numVars = block->numVars();
            numCalleeLocals = block->numCalleeLocals();
            thisRegister = block->thisRegister().offset();
            scopeRegister = block->scopeRegister().offset();
            callFrameCalleeRegister = static_cast<int32_t>(JSC::CallFrameSlot::callee);
            callFrameThisArgumentRegister = JSC::CallFrame::thisArgumentOffset();
            callFrameFirstArgumentRegister = JSC::CallFrame::argumentOffset(0);
            instructionBytes = block->instructionsSize();
        }
        if (!Bun__Hare__visitorBeginFunction(
                visitorContext, functionId, parent, relation, relationIndex,
                specialization, parseMode, scriptMode, codeType, lexicalFeatures,
                codeFeatures, numParameters, numVars, numCalleeLocals,
                thisRegister, scopeRegister, callFrameCalleeRegister,
                callFrameThisArgumentRegister,
                callFrameFirstArgumentRegister, instructionBytes))
            return result(ImportStatus::VisitorRejected, 2);
        if (!visitSwitchTables(*records[functionId]->block.get(), visitorContext))
            return result(ImportStatus::VisitorRejected, 7);
        if (!visitExceptionHandlers(*records[functionId]->block.get(), visitorContext))
            return result(ImportStatus::VisitorRejected, 8);
        if (!visitConstantsAndIdentifiers(
                *records[functionId]->block.get(), visitorContext))
            return result(ImportStatus::VisitorRejected, 6);

        uint32_t offset = 0;
        while (offset < instructionBytes) {
            uint32_t opcodeId;
            uint32_t encodedSize;
            uint32_t opcodeIdBytes;
            uint32_t widthBytes;
            {
                auto* block = records[functionId]->block.get();
                auto instruction = block->instructions().at(offset);
                opcodeId = static_cast<uint32_t>(instruction->opcodeID());
                encodedSize = static_cast<uint32_t>(instruction->size());
                opcodeIdBytes = instruction->opcodeIDBytes();
                widthBytes = 1U << instruction->sizeShiftAmount();
            }
            if (!encodedSize || encodedSize > instructionBytes - offset)
                return result(ImportStatus::VisitorRejected, 3);
            if (!Bun__Hare__visitorInstruction(
                    visitorContext, offset, opcodeId, encodedSize,
                    opcodeIdBytes, widthBytes))
                return result(ImportStatus::VisitorRejected, 4);
            {
                auto* block = records[functionId]->block.get();
                auto instruction = block->instructions().at(offset);
                if (!visitInstructionOperands(*block, instruction, visitorContext))
                    return result(ImportStatus::VisitorRejected, 5);
            }
            offset += encodedSize;
        }
    }

    if (Bun__Hare__nativeProbe(0) != nativeProbeExpected)
        return result(ImportStatus::NativeProbeFailed);
    return result(ImportStatus::Success);
}

template<typename CodeBlockType>
static ImportResult importRoot(
    JSC::VM& vm, const JSC::SourceCode& source, const JSC::SourceCodeKey& sourceKey,
    CodeBlockType& root, Specialization specialization, void* visitorContext)
{
    static_cast<void>(sourceKey);
    auto exceptionScope = DECLARE_TOP_EXCEPTION_SCOPE(vm);
    if (exceptionScope.exception()) {
        (void)exceptionScope.tryClearException();
        return result(ImportStatus::PendingJscException);
    }

    JSC::DeferGC deferGC(vm);
    FunctionRecords records;
    records.append(std::make_unique<FunctionRecord>(
        vm, &root, source, noParent, Relation::Root, 0, specialization));
    auto materializeResult = materializeChildren(vm, records, 0, visitorContext);
    if (materializeResult.status != ImportStatus::Success)
        return materializeResult;
    auto importResult = visitRecords(vm, records, visitorContext);
    if (exceptionScope.exception()) {
        (void)exceptionScope.tryClearException();
        return result(ImportStatus::PendingJscException, 2);
    }
    return importResult;
}

template<typename Callback>
static ImportResult exceptionBoundary(Callback&& callback) noexcept
{
#if defined(__cpp_exceptions)
    try {
        return callback();
    } catch (const std::bad_alloc&) {
        Bun__outOfMemory();
    } catch (...) {
        return result(ImportStatus::InternalCxxException);
    }
#else
    return callback();
#endif
}

ImportResult importProgramForHare(
    JSC::VM& vm, const JSC::SourceCode& source, const JSC::SourceCodeKey& sourceKey,
    JSC::UnlinkedProgramCodeBlock& root, void* visitorContext) noexcept
{
    return exceptionBoundary([&] {
        return importRoot(vm, source, sourceKey, root, Specialization::Program, visitorContext);
    });
}

ImportResult importModuleForHare(
    JSC::VM& vm, const JSC::SourceCode& source, const JSC::SourceCodeKey& sourceKey,
    JSC::UnlinkedModuleProgramCodeBlock& root, void* visitorContext) noexcept
{
    return exceptionBoundary([&] {
        return importRoot(vm, source, sourceKey, root, Specialization::Module, visitorContext);
    });
}

ImportResult importDirectEvalForHare(
    JSC::VM& vm, const JSC::SourceCode& source, const JSC::SourceCodeKey& sourceKey,
    JSC::UnlinkedEvalCodeBlock& root, void* visitorContext) noexcept
{
    return exceptionBoundary([&] {
        return importRoot(vm, source, sourceKey, root, Specialization::Eval, visitorContext);
    });
}

ImportResult importFunctionExecutableForHare(
    JSC::VM& vm, const JSC::SourceCode& source, const JSC::SourceCodeKey& sourceKey,
    JSC::UnlinkedFunctionExecutable& executable,
    const FunctionSpecializationPlan& plan, void* visitorContext) noexcept
{
    return exceptionBoundary([&] {
        static_cast<void>(sourceKey);
        auto exceptionScope = DECLARE_TOP_EXCEPTION_SCOPE(vm);
        if (exceptionScope.exception()) {
            (void)exceptionScope.tryClearException();
            return result(ImportStatus::PendingJscException);
        }
        if (!plan.count || plan.count > 2)
            return result(ImportStatus::InvalidSpecializationPlan, 1);
        if (plan.count == 2 && plan.ordered[0] == plan.ordered[1])
            return result(ImportStatus::InvalidSpecializationPlan, 2);

        JSC::DeferGC deferGC(vm);
        JSC::Strong<JSC::UnlinkedFunctionExecutable> executableRoot(vm, &executable);
        FunctionRecords records;
        for (uint32_t order = 0; order < plan.count; ++order) {
            auto requested = plan.ordered[order];
            if (requested == FunctionSpecialization::Construct
                && executable.constructAbility() != JSC::ConstructAbility::CanConstruct)
                return result(ImportStatus::InvalidSpecializationPlan, 3);
            auto codeKind = requested == FunctionSpecialization::Construct
                ? JSC::CodeSpecializationKind::CodeForConstruct
                : JSC::CodeSpecializationKind::CodeForCall;
            JSC::ParserError parserError;
            auto* block = executable.unlinkedCodeBlockFor(
                vm, source, codeKind, { }, parserError, executable.parseMode());
            if (parserError.isValid())
                return emitParserError(visitorContext, parserError);
            if (!block)
                return result(ImportStatus::NullRoot, 2);
            uint32_t functionId = static_cast<uint32_t>(records.size());
            records.append(std::make_unique<FunctionRecord>(
                vm, block, source, noParent,
                Relation::FunctionConstructorSpecialization, order,
                requested == FunctionSpecialization::Construct
                    ? Specialization::FunctionConstructorConstruct
                    : Specialization::FunctionConstructorCall));
            auto materializeResult = materializeChildren(
                vm, records, functionId, visitorContext);
            if (materializeResult.status != ImportStatus::Success)
                return materializeResult;
        }
        static_cast<void>(executableRoot);
        auto importResult = visitRecords(vm, records, visitorContext);
        if (exceptionScope.exception()) {
            (void)exceptionScope.tryClearException();
            return result(ImportStatus::PendingJscException, 2);
        }
        return importResult;
    });
}

static void destroyBuildVM(RefPtr<JSC::VM>& vm)
{
    JSC::JSLockHolder teardownLock(vm.get());
    vm = nullptr;
}

template<typename RootType, typename Generate, typename MakeKey, typename Import>
static ImportResult importFreshBuildVM(
    BunString* sourceProviderURL, const uint8_t* inputSourceCode,
    size_t inputSourceCodeSize, void* visitorContext, Generate&& generate,
    MakeKey&& makeKey, Import&& import) noexcept
{
    return exceptionBoundary([&] {
        std::span<const Latin1Character> sourceSpan(inputSourceCode, inputSourceCodeSize);
        JSC::SourceCode source = JSC::makeSource(
            WTF::String(sourceSpan), Zig::toSourceOrigin(sourceProviderURL->toWTFString(), false),
            JSC::SourceTaintedOrigin::Untainted);
        RefPtr<JSC::VM> vm = JSC::VM::tryCreate(JSC::HeapType::Small);
        if (!vm)
            Bun__outOfMemory();

        ImportResult importResult;
        {
            JSC::JSLockHolder lock(*vm);
            JSC::ParserError parserError;
            RootType* root = generate(*vm, source, parserError);
            if (parserError.isValid())
                importResult = emitParserError(visitorContext, parserError);
            else if (!root)
                importResult = result(ImportStatus::NullRoot);
            else {
                auto key = makeKey(*vm, source);
                importResult = import(*vm, source, key, *root, visitorContext);
            }
        }
        destroyBuildVM(vm);
        return importResult;
    });
}

} // namespace Bun::Hare

extern "C" void Bun__Hare__importModuleFromSource(
    BunString* sourceProviderURL, const uint8_t* inputSourceCode,
    size_t inputSourceCodeSize, void* visitorContext,
    Bun::Hare::ImportResult* output) noexcept
{
    if (!output)
        return;
    *output = Bun::Hare::importFreshBuildVM<JSC::UnlinkedModuleProgramCodeBlock>(
        sourceProviderURL, inputSourceCode, inputSourceCodeSize, visitorContext,
        [](JSC::VM& vm, const JSC::SourceCode& source, JSC::ParserError& error) {
            return JSC::recursivelyGenerateUnlinkedCodeBlockForModuleProgram(
                vm, source, JSC::StrictModeLexicallyScopedFeature,
                JSC::JSParserScriptMode::Module, { }, error, JSC::EvalContextType::None);
        },
        [](JSC::VM& vm, const JSC::SourceCode& source) {
            return JSC::sourceCodeKeyForSerializedModule(vm, source);
        },
        Bun::Hare::importModuleForHare);
}

extern "C" void Bun__Hare__importProgramFromSource(
    BunString* sourceProviderURL, const uint8_t* inputSourceCode,
    size_t inputSourceCodeSize, void* visitorContext,
    Bun::Hare::ImportResult* output) noexcept
{
    if (!output)
        return;
    *output = Bun::Hare::importFreshBuildVM<JSC::UnlinkedProgramCodeBlock>(
        sourceProviderURL, inputSourceCode, inputSourceCodeSize, visitorContext,
        [](JSC::VM& vm, const JSC::SourceCode& source, JSC::ParserError& error) {
            return JSC::recursivelyGenerateUnlinkedCodeBlockForProgram(
                vm, source, JSC::NoLexicallyScopedFeatures,
                JSC::JSParserScriptMode::Classic, { }, error, JSC::EvalContextType::None);
        },
        [](JSC::VM& vm, const JSC::SourceCode& source) {
            return JSC::sourceCodeKeyForSerializedProgram(vm, source);
        },
        Bun::Hare::importProgramForHare);
}

#if !defined(_WIN32)
extern "C" int32_t Bun__Hare__nativeApplicationEntry(
    int32_t argc, const char* const* argv) noexcept __attribute__((weak));
#endif

extern "C" int32_t Bun__Hare__runNativeApplication(
    int32_t argc, const char* const* argv) noexcept
{
#if defined(_WIN32)
    (void)argc;
    (void)argv;
    return Bun__Hare__nativeApplicationMissing;
#else
    if (!Bun__Hare__nativeApplicationEntry)
        return Bun__Hare__nativeApplicationMissing;
    return Bun__Hare__nativeApplicationEntry(argc, argv);
#endif
}

extern "C" int64_t Bun__Hare__writeStdout(
    const uint8_t* bytes, size_t length) noexcept
{
    if ((!bytes && length) || length > static_cast<size_t>(std::numeric_limits<int64_t>::max()))
        return -1;

    size_t written = 0;
    while (written < length) {
        size_t count = std::fwrite(bytes + written, 1, length - written, stdout);
        if (!count)
            return -1;
        written += count;
    }
    if (std::fflush(stdout))
        return -1;
    return static_cast<int64_t>(written);
}

extern "C" int64_t Bun__Hare__writeInt64Line(int64_t value) noexcept
{
    char buffer[32];
    int length = std::snprintf(buffer, sizeof(buffer), "%lld\n", static_cast<long long>(value));
    if (length < 0 || static_cast<size_t>(length) >= sizeof(buffer))
        return -1;
    return Bun__Hare__writeStdout(
        reinterpret_cast<const uint8_t*>(buffer), static_cast<size_t>(length));
}
