#pragma once

#include <cstddef>
#include <cstdint>

namespace JSC {
class SourceCode;
class SourceCodeKey;
class UnlinkedCodeBlock;
class UnlinkedEvalCodeBlock;
class UnlinkedFunctionExecutable;
class UnlinkedModuleProgramCodeBlock;
class UnlinkedProgramCodeBlock;
class VM;
}

struct BunString;

namespace Bun::Hare {

enum class ImportStatus : uint32_t {
    Success = 0,
    ParserError = 1,
    NullRoot = 2,
    VisitorRejected = 3,
    InternalCxxException = 4,
    InvalidSpecializationPlan = 5,
    NativeProbeFailed = 6,
    PendingJscException = 7,
};

struct ImportResult {
    ImportStatus status { ImportStatus::Success };
    uint32_t detail { 0 };
};

enum class FunctionSpecialization : uint32_t {
    Call = 0,
    Construct = 1,
};

struct FunctionSpecializationPlan {
    uint32_t count { 0 };
    FunctionSpecialization ordered[2] { FunctionSpecialization::Call, FunctionSpecialization::Construct };
};

ImportResult importProgramForHare(
    JSC::VM&, const JSC::SourceCode&, const JSC::SourceCodeKey&,
    JSC::UnlinkedProgramCodeBlock&, void* visitorContext) noexcept;

ImportResult importModuleForHare(
    JSC::VM&, const JSC::SourceCode&, const JSC::SourceCodeKey&,
    JSC::UnlinkedModuleProgramCodeBlock&, void* visitorContext) noexcept;

ImportResult importDirectEvalForHare(
    JSC::VM&, const JSC::SourceCode&, const JSC::SourceCodeKey&,
    JSC::UnlinkedEvalCodeBlock&, void* visitorContext) noexcept;

ImportResult importFunctionExecutableForHare(
    JSC::VM&, const JSC::SourceCode&, const JSC::SourceCodeKey&,
    JSC::UnlinkedFunctionExecutable&, const FunctionSpecializationPlan&,
    void* visitorContext) noexcept;

} // namespace Bun::Hare

extern "C" void Bun__Hare__importModuleFromSource(
    BunString*, const uint8_t*, size_t, void*, Bun::Hare::ImportResult*) noexcept;
extern "C" void Bun__Hare__importProgramFromSource(
    BunString*, const uint8_t*, size_t, void*, Bun::Hare::ImportResult*) noexcept;

static_assert(sizeof(Bun::Hare::ImportResult) == 8);
static_assert(alignof(Bun::Hare::ImportResult) == alignof(uint32_t));
