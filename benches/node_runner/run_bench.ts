import { z } from "zod";
import * as v from "valibot";
import vine from "@vinejs/vine";

// Payloads
const flatValid = { username: "johndoe", password: "securepassword123" };
const flatInvalid = { username: "j", password: "123" };

const nestedValid = {
  username: "johndoe",
  password: "securepassword123",
  contact: { name: "John Doe", address: "123 Tech Blvd" },
};
const nestedInvalid = {
  username: "j",
  password: "123",
  contact: { name: "", address: "123 Tech Blvd" },
};

const arrayValid = {
  contacts: [
    { type: "email", value: "john@example.com" },
    { type: "phone", value: "+1234567890" },
  ],
};

// 1. ZOD SCHEMAS
const zodFlat = z.object({
  username: z.string().min(3).max(30),
  password: z.string().min(8).max(64),
});

const zodNested = z.object({
  username: z.string().min(3).max(30),
  password: z.string().min(8).max(64),
  contact: z.object({
    name: z.string().min(1),
    address: z.string().optional(),
  }),
});

const zodArray = z.object({
  contacts: z.array(
    z.object({
      type: z.string(),
      value: z.string(),
    })
  ),
});

// 2. VALIBOT SCHEMAS
const valibotFlat = v.object({
  username: v.pipe(v.string(), v.minLength(3), v.maxLength(30)),
  password: v.pipe(v.string(), v.minLength(8), v.maxLength(64)),
});

const valibotNested = v.object({
  username: v.pipe(v.string(), v.minLength(3), v.maxLength(30)),
  password: v.pipe(v.string(), v.minLength(8), v.maxLength(64)),
  contact: v.object({
    name: v.pipe(v.string(), v.minLength(1)),
    address: v.optional(v.string()),
  }),
});

const valibotArray = v.object({
  contacts: v.array(
    v.object({
      type: v.string(),
      value: v.string(),
    })
  ),
});

// 3. VINE SCHEMAS
const vineFlat = vine.compile(
  vine.object({
    username: vine.string().minLength(3).maxLength(30),
    password: vine.string().minLength(8).maxLength(64),
  })
);

const vineNested = vine.compile(
  vine.object({
    username: vine.string().minLength(3).maxLength(30),
    password: vine.string().minLength(8).maxLength(64),
    contact: vine.object({
      name: vine.string().minLength(1),
      address: vine.string().optional(),
    }),
  })
);

const vineArray = vine.compile(
  vine.object({
    contacts: vine.array(
      vine.object({
        type: vine.string(),
        value: vine.string(),
      })
    ),
  })
);

function bench(name: string, fn: () => void, iterations: number = 200_000) {
  // Warmup
  for (let i = 0; i < 20_000; i++) {
    try { fn(); } catch {}
  }
  const start = performance.now();
  for (let i = 0; i < iterations; i++) {
    try { fn(); } catch {}
  }
  const end = performance.now();
  const totalMs = end - start;
  const nsPerOp = (totalMs * 1_000_000) / iterations;
  const opsPerSec = (iterations / (totalMs / 1000));
  console.log(`${name.padEnd(25)}: ${nsPerOp.toFixed(1).padStart(8)} ns/op | ${opsPerSec.toLocaleString("en-US", { maximumFractionDigits: 0 }).padStart(14)} ops/sec`);
  return { nsPerOp, opsPerSec };
}

async function benchAsync(name: string, fn: () => Promise<any>, iterations: number = 100_000) {
  // Warmup
  for (let i = 0; i < 5_000; i++) {
    try { await fn(); } catch {}
  }
  const start = performance.now();
  for (let i = 0; i < iterations; i++) {
    try { await fn(); } catch {}
  }
  const end = performance.now();
  const totalMs = end - start;
  const nsPerOp = (totalMs * 1_000_000) / iterations;
  const opsPerSec = (iterations / (totalMs / 1000));
  console.log(`${name.padEnd(25)}: ${nsPerOp.toFixed(1).padStart(8)} ns/op | ${opsPerSec.toLocaleString("en-US", { maximumFractionDigits: 0 }).padStart(14)} ops/sec`);
  return { nsPerOp, opsPerSec };
}

async function main() {
  console.log("=== Node / Bun Ecosystem Validator Benchmark ===");
  console.log("Testing sync parsing:");
  bench("Zod (Flat Valid)", () => zodFlat.safeParse(flatValid));
  bench("Valibot (Flat Valid)", () => v.safeParse(valibotFlat, flatValid));
  
  bench("Zod (Flat Invalid)", () => zodFlat.safeParse(flatInvalid));
  bench("Valibot (Flat Invalid)", () => v.safeParse(valibotFlat, flatInvalid));

  bench("Zod (Nested Valid)", () => zodNested.safeParse(nestedValid));
  bench("Valibot (Nested Valid)", () => v.safeParse(valibotNested, nestedValid));

  bench("Zod (Array Valid)", () => zodArray.safeParse(arrayValid));
  bench("Valibot (Array Valid)", () => v.safeParse(valibotArray, arrayValid));

  console.log("\nTesting async parsing (including VineJS):");
  await benchAsync("VineJS (Flat Valid)", () => vineFlat.validate(flatValid));
  await benchAsync("Zod Async (Flat Valid)", () => zodFlat.safeParseAsync(flatValid));
  await benchAsync("Valibot Async (Flat Valid)", () => v.safeParseAsync(valibotFlat, flatValid));

  await benchAsync("VineJS (Nested Valid)", () => vineNested.validate(nestedValid));
  await benchAsync("Zod Async (Nested Valid)", () => zodNested.safeParseAsync(nestedValid));
  await benchAsync("Valibot Async (Nested Valid)", () => v.safeParseAsync(valibotNested, nestedValid));
}

main();
