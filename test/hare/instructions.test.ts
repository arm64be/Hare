import { expect, test } from "bun:test";
import { bunEnv, bunExe, isLinux, tempDir } from "harness";
import { closeSync, openSync, readSync } from "node:fs";
import { join } from "node:path";
import cases from "./instructions/cases.json";

function readAt(fd: number, offset: number, length: number): Buffer {
  const bytes = Buffer.alloc(length);
  expect(readSync(fd, bytes, 0, length, offset)).toBe(length);
  return bytes;
}

function readHareGraphFlags(executable: string): number {
  const fd = openSync(executable, "r");
  try {
    const elf = readAt(fd, 0, 64);
    expect(elf.readUInt32BE(0)).toBe(0x7f454c46);
    const sectionTableOffset = Number(elf.readBigUInt64LE(40));
    const sectionEntrySize = elf.readUInt16LE(58);
    const sectionCount = elf.readUInt16LE(60);
    const namesIndex = elf.readUInt16LE(62);
    const sectionTable = readAt(fd, sectionTableOffset, sectionEntrySize * sectionCount);
    const namesHeader = namesIndex * sectionEntrySize;
    const namesOffset = Number(sectionTable.readBigUInt64LE(namesHeader + 24));
    const namesLength = Number(sectionTable.readBigUInt64LE(namesHeader + 32));
    const names = readAt(fd, namesOffset, namesLength);
    for (let index = 0; index < sectionCount; index++) {
      const header = index * sectionEntrySize;
      const nameOffset = sectionTable.readUInt32LE(header);
      const nameEnd = names.indexOf(0, nameOffset);
      if (names.subarray(nameOffset, nameEnd).toString() !== ".bun") continue;
      const sectionOffset = Number(sectionTable.readBigUInt64LE(header + 24));
      const sectionLength = Number(sectionTable.readBigUInt64LE(header + 32));
      const section = readAt(fd, sectionOffset, sectionLength);
      const graphLength = Number(section.readBigUInt64LE(0));
      const graph = section.subarray(8, 8 + graphLength);
      const trailerSize = Buffer.byteLength("\n---- Bun! ----\n");
      return graph.readUInt32LE(graph.length - trailerSize - 4);
    }
    throw new Error("compiled executable has no .bun section");
  } finally {
    closeSync(fd);
  }
}

test.skipIf(!isLinux)(
  "Hare native scalar convergence matches the pinned JSC frontend",
  async () => {
    const opcodeLines = (await Bun.file(join(import.meta.dir, "../../OPCODES.tsv")).text()).trimEnd().split("\n");
    const opcodeHeaders = opcodeLines.shift()!.split("\t");
    const opcodeNameIndex = opcodeHeaders.indexOf("opcode");
    const opcodeIdIndex = opcodeHeaders.indexOf("opcode_id");
    const classificationIndex = opcodeHeaders.indexOf("classification");
    const semanticOpcodesById = new Map(
      opcodeLines
        .map(line => line.split("\t"))
        .filter(columns => columns[classificationIndex] === "semantic")
        .map(columns => [Number(columns[opcodeIdIndex]), columns[opcodeNameIndex]]),
    );
    const source = cases.map(testCase => `// ${testCase.id}\n${testCase.source}`).join("\n");
    using dir = tempDir("hare-instruction-differential", { "reference.js": source });

    await using reference = Bun.spawn({
      cmd: [bunExe(), "reference.js"],
      cwd: String(dir),
      env: bunEnv,
      stdout: "pipe",
      stderr: "pipe",
    });
    const [referenceStdout, referenceStderr, referenceExitCode] = await Promise.all([
      reference.stdout.text(),
      reference.stderr.text(),
      reference.exited,
    ]);
    expect(referenceStderr).toBe("");
    expect(referenceStdout).toMatchInlineSnapshot(`
      "42
      42
      2
      1
      NaN
      -0
      42
      2
      -3
      13
      19
      13
      12
      -4
      2147483647
      -1
      41
      64
      7
      true
      true
      true
      true
      true
      true
      true
      true
      true
      false
      false
      false
      false
      true
      false
      false
      true
      false
      true
      15
      number
      12
      5
      6
      9
      0
      1
      0
      1
      1
      1
      0
      0
      0
      0
      1
      1
      0
      0
      1
      2
      3
      5
      20
      2
      8
      11
      12
      hare-42
      42
      true
      true
      40
      true
      true
      42
      7
      3
      7
      12
      0
      42
      42
      42
      42
      42
      42

      "
    `);
    expect(referenceExitCode).toBe(0);

    let nativeStdout = "";
    const lowerings = [...new Set(cases.map(testCase => testCase.lowering))];
    for (const lowering of lowerings) {
      const groupSource = cases
        .filter(testCase => testCase.lowering === lowering)
        .map(testCase => `// ${testCase.id}\n${testCase.source}`)
        .join("\n");
      const entrypoint = join(String(dir), `${lowering}.js`);
      await Bun.write(entrypoint, groupSource);
      const executable = join(String(dir), `${lowering}-app`);
      await using compile = Bun.spawn({
        cmd: [bunExe(), "build", "--compile", "--hare", entrypoint, "--outfile", executable],
        cwd: String(dir),
        env: { ...bunEnv, BUN_DEBUG_HARE_IR: "1" },
        stdout: "pipe",
        stderr: "pipe",
      });
      const [, compileStderr, compileExitCode] = await Promise.all([
        compile.stdout.text(),
        compile.stderr.text(),
        compile.exited,
      ]);
      const compileLines = compileStderr.split("\n");
      const compileError = compileLines.find(line => line.startsWith("error:"));
      const failedFunctionId = compileError?.match(/\bf\d+\b/)?.[0];
      const failedFunctionStart = failedFunctionId
        ? compileLines.findIndex(line => line.startsWith(`function ${failedFunctionId} `))
        : -1;
      const failedFunctionEnd = compileLines.findIndex(
        (line, index) => index > failedFunctionStart && line.startsWith("function "),
      );
      const failedFunctionDump =
        failedFunctionStart === -1
          ? ""
          : compileLines
              .slice(failedFunctionStart, failedFunctionEnd === -1 ? undefined : failedFunctionEnd)
              .join("\n");
      expect({ compileError, failedFunctionDump }).toEqual({ compileError: undefined, failedFunctionDump: "" });
      expect(compileStderr).toContain("hare-dump schema=1");
      expect(compileStderr).toContain("structurally_complete=true");
      expect(compileStderr).toMatch(/simple-switch t\d+ minimum=2147483647 default=-?\d+ list=true offsets=/);
      const claimedOpcodes = new Set(
        cases.filter(testCase => testCase.lowering === lowering).flatMap(testCase => testCase.opcodes),
      );
      const dumpedOpcodes = new Set(
        [...compileStderr.matchAll(/^  instruction offset=\d+ opcode=(\d+) /gm)]
          .map(match => semanticOpcodesById.get(Number(match[1])))
          .filter((opcode): opcode is string => opcode !== undefined),
      );
      expect([...claimedOpcodes].filter(opcode => !dumpedOpcodes.has(opcode)).sort()).toEqual([]);
      expect([...dumpedOpcodes].filter(opcode => !claimedOpcodes.has(opcode)).sort()).toEqual([]);
      expect(readHareGraphFlags(executable) & (1 << 4)).toBe(1 << 4);
      expect(compileExitCode).toBe(0);

      await using native = Bun.spawn({
        cmd: [executable],
        cwd: String(dir),
        env: bunEnv,
        stdout: "pipe",
        stderr: "pipe",
      });
      const [stdout, stderr, exitCode] = await Promise.all([native.stdout.text(), native.stderr.text(), native.exited]);
      expect(stderr).toBe("");
      nativeStdout += stdout;
      expect(exitCode).toBe(0);
    }
    expect(nativeStdout).toBe(referenceStdout);
  },
  180_000,
);
