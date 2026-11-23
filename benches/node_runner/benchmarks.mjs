import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';
import { z } from 'zod';
import * as v from 'valibot';
import vine from '@vinejs/vine';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const corpusPath = path.resolve(__dirname, '../data/corpus.json');
const corpus = JSON.parse(fs.readFileSync(corpusPath, 'utf8'));

// Benchmark Runner
function benchmark(name, data, fn, targetOps = 100000) {
    const iterations = Math.max(1, Math.ceil(targetOps / data.length));
    
    // Warmup
    for (let i = 0; i < 5; i++) {
        for (let j = 0; j < data.length; j++) {
            try { fn(data[j]); } catch {}
        }
    }

    const start = process.hrtime.bigint();
    let count = 0;
    for (let i = 0; i < iterations; i++) {
        for (let j = 0; j < data.length; j++) {
            try { fn(data[j]); } catch {}
            count++;
        }
    }
    const end = process.hrtime.bigint();
    const durationNs = Number(end - start);
    const durationMs = durationNs / 1e6;
    const durationSec = durationNs / 1e9;
    const nsPerOp = durationNs / count;
    const mOpsPerSec = (count / durationSec) / 1e6;

    return {
        name,
        total_ops: count,
        duration_ms: durationMs,
        ns_per_op: nsPerOp,
        m_ops_per_sec: mOpsPerSec
    };
}

// Async Benchmark Runner (for VineJS which returns a Promise)
async function benchmarkAsync(name, data, fn, targetOps = 100000) {
    const iterations = Math.max(1, Math.ceil(targetOps / data.length));
    
    // Warmup
    for (let i = 0; i < 3; i++) {
        for (let j = 0; j < data.length; j++) {
            try { await fn(data[j]); } catch {}
        }
    }

    const start = process.hrtime.bigint();
    let count = 0;
    for (let i = 0; i < iterations; i++) {
        for (let j = 0; j < data.length; j++) {
            try { await fn(data[j]); } catch {}
            count++;
        }
    }
    const end = process.hrtime.bigint();
    const durationNs = Number(end - start);
    const durationMs = durationNs / 1e6;
    const durationSec = durationNs / 1e9;
    const nsPerOp = durationNs / count;
    const mOpsPerSec = (count / durationSec) / 1e6;

    return {
        name,
        total_ops: count,
        duration_ms: durationMs,
        ns_per_op: nsPerOp,
        m_ops_per_sec: mOpsPerSec
    };
}

async function runAll() {
    const results = {};

    // 1. FLAT SCHEMAS
    const zodFlat = z.object({
        username: z.string().min(3).max(30),
        email: z.string().email(),
        age: z.number().int().min(18).max(100),
    });

    const valibotFlat = v.object({
        username: v.pipe(v.string(), v.minLength(3), v.maxLength(30)),
        email: v.pipe(v.string(), v.email()),
        age: v.pipe(v.number(), v.integer(), v.minValue(18), v.maxValue(100)),
    });

    const vineFlat = vine.compile(
        vine.object({
            username: vine.string().minLength(3).maxLength(30),
            email: vine.string().email(),
            age: vine.number().min(18).max(100),
        })
    );

    results['flat_valid'] = [
        benchmark('Zod (sync)', corpus.flat_valid, (item) => zodFlat.safeParse(item), 100000),
        benchmark('Valibot (sync)', corpus.flat_valid, (item) => v.safeParse(valibotFlat, item), 100000),
        await benchmarkAsync('VineJS (async)', corpus.flat_valid, (item) => vineFlat.validate(item), 100000),
    ];

    results['flat_invalid'] = [
        benchmark('Zod (sync)', corpus.flat_invalid, (item) => zodFlat.safeParse(item), 100000),
        benchmark('Valibot (sync)', corpus.flat_invalid, (item) => v.safeParse(valibotFlat, item), 100000),
        await benchmarkAsync('VineJS (async)', corpus.flat_invalid, (item) => vineFlat.validate(item), 100000),
    ];

    // 2. NESTED SCHEMAS
    const zodNested = z.object({
        username: z.string().min(3).max(30),
        email: z.string().email(),
        profile: z.object({
            full_name: z.string().min(1).max(100),
            city: z.string().min(1).max(50),
        }),
    });

    const valibotNested = v.object({
        username: v.pipe(v.string(), v.minLength(3), v.maxLength(30)),
        email: v.pipe(v.string(), v.email()),
        profile: v.object({
            full_name: v.pipe(v.string(), v.minLength(1), v.maxLength(100)),
            city: v.pipe(v.string(), v.minLength(1), v.maxLength(50)),
        }),
    });

    const vineNested = vine.compile(
        vine.object({
            username: vine.string().minLength(3).maxLength(30),
            email: vine.string().email(),
            profile: vine.object({
                full_name: vine.string().minLength(1).maxLength(100),
                city: vine.string().minLength(1).maxLength(50),
            }),
        })
    );

    results['nested_valid'] = [
        benchmark('Zod (sync)', corpus.nested_valid, (item) => zodNested.safeParse(item), 100000),
        benchmark('Valibot (sync)', corpus.nested_valid, (item) => v.safeParse(valibotNested, item), 100000),
        await benchmarkAsync('VineJS (async)', corpus.nested_valid, (item) => vineNested.validate(item), 100000),
    ];

    results['nested_invalid'] = [
        benchmark('Zod (sync)', corpus.nested_invalid, (item) => zodNested.safeParse(item), 100000),
        benchmark('Valibot (sync)', corpus.nested_invalid, (item) => v.safeParse(valibotNested, item), 100000),
        await benchmarkAsync('VineJS (async)', corpus.nested_invalid, (item) => vineNested.validate(item), 100000),
    ];

    // 3. ARRAY OF OBJECTS
    const zodArray = z.array(
        z.object({
            name: z.string().min(1).max(50),
            email: z.string().email(),
            score: z.number().int().min(0).max(100),
        })
    );

    const valibotArray = v.array(
        v.object({
            name: v.pipe(v.string(), v.minLength(1), v.maxLength(50)),
            email: v.pipe(v.string(), v.email()),
            score: v.pipe(v.number(), v.integer(), v.minValue(0), v.maxValue(100)),
        })
    );

    const vineArray = vine.compile(
        vine.array(
            vine.object({
                name: vine.string().minLength(1).maxLength(50),
                email: vine.string().email(),
                score: vine.number().min(0).max(100),
            })
        )
    );

    results['array_valid'] = [
        benchmark('Zod (sync)', corpus.array_valid, (item) => zodArray.safeParse(item), 50000),
        benchmark('Valibot (sync)', corpus.array_valid, (item) => v.safeParse(valibotArray, item), 50000),
        await benchmarkAsync('VineJS (async)', corpus.array_valid, (item) => vineArray.validate(item), 50000),
    ];

    results['array_invalid'] = [
        benchmark('Zod (sync)', corpus.array_invalid, (item) => zodArray.safeParse(item), 50000),
        benchmark('Valibot (sync)', corpus.array_invalid, (item) => v.safeParse(valibotArray, item), 50000),
        await benchmarkAsync('VineJS (async)', corpus.array_invalid, (item) => vineArray.validate(item), 50000),
    ];

    // 4. PRIMITIVE EMAIL
    const zodEmail = z.string().email();
    const valibotEmail = v.pipe(v.string(), v.email());
    const vineEmail = vine.compile(vine.string().email());

    results['primitive_email'] = [
        benchmark('Zod (sync)', corpus.primitive_emails_valid, (s) => zodEmail.safeParse(s), 200000),
        benchmark('Valibot (sync)', corpus.primitive_emails_valid, (s) => v.safeParse(valibotEmail, s), 200000),
        await benchmarkAsync('VineJS (async)', corpus.primitive_emails_valid, (s) => vineEmail.validate(s), 200000),
    ];

    results['primitive_email_invalid'] = [
        benchmark('Zod (sync)', corpus.primitive_emails_invalid, (s) => zodEmail.safeParse(s), 200000),
        benchmark('Valibot (sync)', corpus.primitive_emails_invalid, (s) => v.safeParse(valibotEmail, s), 200000),
        await benchmarkAsync('VineJS (async)', corpus.primitive_emails_invalid, (s) => vineEmail.validate(s), 200000),
    ];

    // 5. PRIMITIVE UUID
    const zodUuid = z.string().uuid();
    const valibotUuid = v.pipe(v.string(), v.uuid());
    const vineUuid = vine.compile(vine.string().uuid());

    results['primitive_uuid'] = [
        benchmark('Zod (sync)', corpus.primitive_uuids_valid, (s) => zodUuid.safeParse(s), 200000),
        benchmark('Valibot (sync)', corpus.primitive_uuids_valid, (s) => v.safeParse(valibotUuid, s), 200000),
        await benchmarkAsync('VineJS (async)', corpus.primitive_uuids_valid, (s) => vineUuid.validate(s), 200000),
    ];

    // 6. PRIMITIVE INTEGER
    const zodInt = z.number().int().min(18).max(100);
    const valibotInt = v.pipe(v.number(), v.integer(), v.minValue(18), v.maxValue(100));

    results['primitive_integer'] = [
        benchmark('Zod (sync)', corpus.primitive_integers_valid, (n) => zodInt.safeParse(n), 500000),
        benchmark('Valibot (sync)', corpus.primitive_integers_valid, (n) => v.safeParse(valibotInt, n), 500000),
    ];

    // 7. UNION
    const zodUnion = z.union([
        z.object({ channel: z.literal('email'), address: z.string().email() }),
        z.object({ channel: z.literal('phone'), phone: z.string().min(7) }),
    ]);

    const valibotUnion = v.union([
        v.object({ channel: v.literal('email'), address: v.pipe(v.string(), v.email()) }),
        v.object({ channel: v.literal('phone'), phone: v.pipe(v.string(), v.minLength(7)) }),
    ]);

    results['union_valid'] = [
        benchmark('Zod (sync)', corpus.union_valid, (item) => zodUnion.safeParse(item), 100000),
        benchmark('Valibot (sync)', corpus.union_valid, (item) => v.safeParse(valibotUnion, item), 100000),
    ];

    // 8. PRIMITIVE FLOAT
    const zodFloat = z.number().min(0).max(100);
    const valibotFloat = v.pipe(v.number(), v.minValue(0), v.maxValue(100));
    results['primitive_float'] = [
        benchmark('Zod (sync)', corpus.primitive_floats_valid, (n) => zodFloat.safeParse(n), 500000),
        benchmark('Valibot (sync)', corpus.primitive_floats_valid, (n) => v.safeParse(valibotFloat, n), 500000),
    ];

    // 9. PRIMITIVE BOOLEAN
    const zodBool = z.boolean();
    const valibotBool = v.boolean();
    results['primitive_boolean'] = [
        benchmark('Zod (sync)', corpus.primitive_booleans_valid, (b) => zodBool.safeParse(b), 500000),
        benchmark('Valibot (sync)', corpus.primitive_booleans_valid, (b) => v.safeParse(valibotBool, b), 500000),
    ];

    // 10. STRING RULES
    const zodStr = z.string().startsWith('Prefix_').endsWith('_Suffix').includes('User');
    const valibotStr = v.pipe(v.string(), v.startsWith('Prefix_'), v.endsWith('_Suffix'), v.includes('User'));
    results['string_rules'] = [
        benchmark('Zod (prefix/suffix/contains)', corpus.string_rules_valid, (s) => zodStr.safeParse(s), 200000),
        benchmark('Valibot (prefix/suffix/contains)', corpus.string_rules_valid, (s) => v.safeParse(valibotStr, s), 200000),
    ];

    // 11. RECORDS & MAPS
    const zodRecord = z.record(z.string(), z.number());
    const valibotRecord = v.record(v.string(), v.number());
    results['record_schema'] = [
        benchmark('Zod (record)', corpus.records_valid, (r) => zodRecord.safeParse(r), 100000),
        benchmark('Valibot (record)', corpus.records_valid, (r) => v.safeParse(valibotRecord, r), 100000),
    ];

    // 12. TUPLES
    const zodTuple = z.tuple([z.string(), z.number(), z.boolean()]);
    const valibotTuple = v.tuple([v.string(), v.number(), v.boolean()]);
    results['tuple_schema'] = [
        benchmark('Zod (tuple)', corpus.tuples_valid, (t) => zodTuple.safeParse(t), 100000),
        benchmark('Valibot (tuple)', corpus.tuples_valid, (t) => v.safeParse(valibotTuple, t), 100000),
    ];

    // 13. ENUMS
    const zodEnum = z.enum(['pending', 'active', 'suspended', 'archived']);
    const valibotEnum = v.picklist(['pending', 'active', 'suspended', 'archived']);
    results['enum_schema'] = [
        benchmark('Zod (enum)', corpus.enums_valid, (e) => zodEnum.safeParse(e), 200000),
        benchmark('Valibot (picklist)', corpus.enums_valid, (e) => v.safeParse(valibotEnum, e), 200000),
    ];

    // 14. TEMPORAL (RFC 3339 DATETIME)
    const zodDt = z.string().datetime();
    const valibotDt = v.pipe(v.string(), v.isoTimestamp());
    results['temporal_rfc3339'] = [
        benchmark('Zod (datetime)', corpus.datetimes_valid, (d) => zodDt.safeParse(d), 200000),
        benchmark('Valibot (isoTimestamp)', corpus.datetimes_valid, (d) => v.safeParse(valibotDt, d), 200000),
    ];

    // 15. NETWORK RULES (IPv4 & URL)
    const zodIpv4 = z.string().ipv4();
    const valibotIpv4 = v.pipe(v.string(), v.ipv4());
    const zodUrl = z.string().url();
    const valibotUrl = v.pipe(v.string(), v.url());
    results['network_rules'] = [
        benchmark('Zod (ipv4)', corpus.ipv4_valid, (ip) => zodIpv4.safeParse(ip), 200000),
        benchmark('Valibot (ipv4)', corpus.ipv4_valid, (ip) => v.safeParse(valibotIpv4, ip), 200000),
        benchmark('Zod (url)', corpus.urls_valid, (u) => zodUrl.safeParse(u), 200000),
        benchmark('Valibot (url)', corpus.urls_valid, (u) => v.safeParse(valibotUrl, u), 200000),
    ];

    // 16. IDENTIFIERS (ULID)
    const zodUlid = z.string().ulid();
    const valibotUlid = v.pipe(v.string(), v.ulid());
    results['identifiers'] = [
        benchmark('Zod (ulid)', corpus.ulids_valid, (u) => zodUlid.safeParse(u), 200000),
        benchmark('Valibot (ulid)', corpus.ulids_valid, (u) => v.safeParse(valibotUlid, u), 200000),
    ];

    // 17. CRYPTO & SECURITY (BASE64 & JWT)
    const zodB64 = z.string().base64();
    const valibotB64 = v.pipe(v.string(), v.base64());
    const zodJwt = z.string().jwt();
    results['crypto_and_security'] = [
        benchmark('Zod (base64)', corpus.base64_valid, (s) => zodB64.safeParse(s), 200000),
        benchmark('Valibot (base64)', corpus.base64_valid, (s) => v.safeParse(valibotB64, s), 200000),
        benchmark('Zod (jwt)', corpus.jwt_valid, (s) => zodJwt.safeParse(s), 200000),
    ];

    console.log(JSON.stringify(results, null, 2));
}

runAll().catch(console.error);
