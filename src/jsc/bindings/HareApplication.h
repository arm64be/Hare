#pragma once

#include <cstddef>
#include <cstdint>

extern "C" int32_t Bun__Hare__runNativeApplication(
    int32_t argc, const char* const* argv) noexcept;
extern "C" int64_t Bun__Hare__writeStdout(
    const uint8_t* bytes, size_t length) noexcept;
extern "C" int64_t Bun__Hare__writeInt64Line(int64_t value) noexcept;

static constexpr int32_t Bun__Hare__nativeApplicationMissing = INT32_MIN;
