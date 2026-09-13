#!/usr/bin/env node
/**
 * patch-imports.mjs — 测试环境专用：修补 PE 导入表（请勿提交到正式仓库）。
 *
 * 背景：本机是精简 Windows 镜像，kernel32.dll 缺少 WaitOnAddress /
 * WakeByAddressAll / WakeByAddressSingle（Windows 8+ API）。Rust std 的 futex
 * 实现静态导入这些函数，且 api-set 名 api-ms-win-core-synch-l1-2-0 的 schema
 * 解析直接映射到 kernel32（本地文件无法拦截），导致任何 Rust 程序启动即
 * 0xc0000139（STATUS_ENTRYPOINT_NOT_FOUND）。
 *
 * 本脚本把 exe 导入表里该 api-set 的名字改写为普通 DLL 名
 * （tt-synch-stub.dll），迫使 loader 按文件搜索（exe 所在目录），加载由
 * stub-synch.c 编译的真实实现桩 DLL。
 *
 * 用法：
 *   node scripts/patch-imports.mjs <exe 路径>...
 * 要求：exe 同目录存在 tt-synch-stub.dll（由 gcc 编译 stub-synch.c 生成）。
 */
import fs from 'node:fs';

const MAP = {
  // 精简镜像：kernel32 缺 WaitOnAddress 系列（api-set schema 直连 kernel32，
  // 本地同名文件无法拦截）→ 改名迫使 loader 按文件搜索加载真实实现 stub。
  'api-ms-win-core-synch-l1-2-0.dll': 'tt-synch-stub.dll',
  // 精简镜像：comctl32 v5.82 缺 TaskDialogIndirect → 转发/桩实现。
  'comctl32.dll': 'ttcomctl.dll',
};

function patchFile(path) {
  const buf = fs.readFileSync(path);
  const e_lfanew = buf.readUInt32LE(0x3c);
  const pe = e_lfanew + 4;
  const numSections = buf.readUInt16LE(pe + 2);
  const optSize = buf.readUInt16LE(pe + 16);
  const opt = pe + 20;
  // PE32+ Optional Header: DataDirectory 起始于偏移 112；[1] = Import Table
  const importRVA = buf.readUInt32LE(opt + 120);
  const secStart = opt + optSize;

  const rvaToOff = (rva) => {
    for (let i = 0; i < numSections; i++) {
      const s = secStart + i * 40;
      const va = buf.readUInt32LE(s + 12);
      const vsz = buf.readUInt32LE(s + 8);
      const raw = buf.readUInt32LE(s + 20);
      const rsz = buf.readUInt32LE(s + 16);
      if (rva >= va && rva < va + Math.max(vsz, rsz)) return raw + (rva - va);
    }
    return -1;
  };

  let off = rvaToOff(importRVA);
  if (off < 0) throw new Error(`cannot map import table RVA 0x${importRVA.toString(16)}`);
  let found = 0;
  for (;;) {
    const nameRVA = buf.readUInt32LE(off + 12);
    if (nameRVA === 0) break;
    const nameOff = rvaToOff(nameRVA);
    if (nameOff < 0) throw new Error(`cannot map name RVA 0x${nameRVA.toString(16)}`);
    let end = nameOff;
    while (buf[end] !== 0) end++;
    const name = buf.toString('latin1', nameOff, end);
    const newName = MAP[name];
    if (newName) {
      const n = Buffer.from(newName + '\0', 'latin1');
      if (n.length > end - nameOff + 1) {
        throw new Error(`new name too long for slot at RVA 0x${nameRVA.toString(16)}`);
      }
      n.copy(buf, nameOff);
      found++;
      console.log(`  [${path}] patched import name @RVA 0x${nameRVA.toString(16)} (${name} -> ${newName})`);
    }
    off += 20;
  }
  if (found === 0) {
    console.log(`  [${path}] no mapped imports found — already patched or not needed`);
    return false;
  }
  fs.writeFileSync(path, buf);
  return true;
}

const targets = process.argv.slice(2);
if (targets.length === 0) {
  console.error('usage: node scripts/patch-imports.mjs <exe>...');
  process.exit(2);
}
for (const t of targets) patchFile(t);
console.log('done');
