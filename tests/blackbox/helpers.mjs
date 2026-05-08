import { randomBytes } from 'node:crypto'
import { mkdtemp, rm, writeFile } from 'node:fs/promises'
import { mkdtempSync, rmSync } from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawnSync } from 'node:child_process'

const repoRoot = path.resolve(fileURLToPath(new URL('../..', import.meta.url)))
const packageTempRoot = mkdtempSync(path.join(os.tmpdir(), 'ttid-package-'))
let packedTarball

process.on('exit', () => {
  rmSync(packageTempRoot, { recursive: true, force: true })
})

export function uniqueName(prefix = 'ttid') {
  return `${prefix}-${Date.now()}-${randomBytes(3).toString('hex')}`
}

export function run(command, args, { cwd, env = {}, timeout = 120_000 } = {}) {
  const result = spawnSync(command, args, {
    cwd,
    env: { ...process.env, ...env },
    encoding: 'utf8',
    timeout,
  })

  return {
    status: result.status,
    stdout: result.stdout ?? '',
    stderr: result.stderr ?? '',
    error: result.error,
  }
}

export function assertRun(result, label) {
  if (result.status === 0 && !result.error) return

  throw new Error(
    [
      `${label} failed with status ${result.status}`,
      result.error ? `error: ${result.error.message}` : undefined,
      result.stdout ? `stdout:\n${result.stdout}` : undefined,
      result.stderr ? `stderr:\n${result.stderr}` : undefined,
    ]
      .filter(Boolean)
      .join('\n\n')
  )
}

export function ttidTarball() {
  if (process.env.TTID_PACKAGE_TARBALL) return process.env.TTID_PACKAGE_TARBALL
  if (packedTarball) return packedTarball

  const tarball = path.join(packageTempRoot, `${uniqueName('ttid-package')}.tgz`)
  const pack = run('bun', ['pm', 'pack', '--filename', tarball, '--quiet'], {
    cwd: repoRoot,
  })
  assertRun(pack, `bun pm pack --filename ${tarball}`)
  packedTarball = tarball
  return packedTarball
}

export async function createTtidConsumer() {
  const tarball = ttidTarball()
  const root = await mkdtemp(path.join(os.tmpdir(), 'ttid-consumer-'))

  await writeFile(
    path.join(root, 'package.json'),
    JSON.stringify(
      {
        private: true,
        type: 'module',
        devDependencies: {
          typescript: '^5.0.0',
        },
      },
      null,
      2
    )
  )

  const installTypescript = run('bun', ['install'], { cwd: root })
  assertRun(installTypescript, 'bun install')

  const installPackage = run('bun', ['add', tarball], { cwd: root })
  assertRun(installPackage, `bun add ${tarball}`)

  return {
    root,
    async runModule(source, env = {}) {
      const file = path.join(root, `${uniqueName('script')}.mjs`)
      await writeFile(file, source)

      const result = run('bun', [file], {
        cwd: root,
        env,
      })
      assertRun(result, `bun ${path.basename(file)}`)
      return result
    },
    async typecheck(source) {
      const file = path.join(root, 'consumer.ts')
      await writeFile(file, source)
      await writeFile(
        path.join(root, 'tsconfig.json'),
        JSON.stringify(
          {
            compilerOptions: {
              strict: true,
              target: 'ES2022',
              module: 'ESNext',
              moduleResolution: 'Bundler',
              noEmit: true,
            },
            include: ['consumer.ts'],
          },
          null,
          2
        )
      )

      const tsc = path.join(root, 'node_modules', 'typescript', 'bin', 'tsc')
      const result = run('bun', [tsc, '--noEmit', '-p', root], { cwd: root })
      assertRun(result, 'bun tsc --noEmit')
      return result
    },
    async cleanup() {
      await rm(root, { recursive: true, force: true })
    },
  }
}
