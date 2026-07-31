#include "root.h"

#include "HareCompiler.h"
#include "ZigSourceProvider.h"
#include "helpers.h"

#include <memory>
#include <JavaScriptCore/Instruction.h>

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
#include <JavaScriptCore/ParserError.h>
#include <JavaScriptCore/SourceCodeKey.h>
#include <JavaScriptCore/Strong.h>
#include <JavaScriptCore/StrongInlines.h>
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
    int32_t, int32_t, uint32_t);
extern "C" uint32_t Bun__Hare__visitorInstruction(
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
    const JSC::JSInstruction* instruction, void* visitorContext)
{
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
        auto codeKind = executable->isConstructor()
            ? JSC::CodeSpecializationKind::CodeForConstruct
            : JSC::CodeSpecializationKind::CodeForCall;
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
            executable->isConstructor() ? Specialization::Construct : Specialization::Call));
        return materializeChildren(vm, records, childId, visitorContext);
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
            instructionBytes = block->instructionsSize();
        }
        if (!Bun__Hare__visitorBeginFunction(
                visitorContext, functionId, parent, relation, relationIndex,
                specialization, parseMode, scriptMode, codeType, lexicalFeatures,
                codeFeatures, numParameters, numVars, numCalleeLocals,
                thisRegister, scopeRegister, instructionBytes))
            return result(ImportStatus::VisitorRejected, 2);

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
                if (!visitInstructionOperands(instruction.ptr(), visitorContext))
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
