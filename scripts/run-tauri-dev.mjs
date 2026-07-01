import { spawn, spawnSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const port = 1420;

cleanupPreviousDevRun();
startTauriDev();

function cleanupPreviousDevRun() {
  if (process.platform !== 'win32') {
    return;
  }

  const script = `
$ErrorActionPreference = 'SilentlyContinue'
$repo = ${toPowerShellString(repoRoot)}
$port = ${port}
$ids = @()

$connections = Get-NetTCPConnection -LocalPort $port -State Listen -ErrorAction SilentlyContinue
foreach ($connection in $connections) {
  $process = Get-CimInstance Win32_Process -Filter "ProcessId=$($connection.OwningProcess)"
  if ($process -and (($process.CommandLine -like "*$repo*") -or ($process.ExecutablePath -like "$repo*"))) {
    $ids += [int]$connection.OwningProcess
  }
}

$processes = Get-CimInstance Win32_Process | Where-Object {
  (($_.ExecutablePath -like "$repo*") -and ($_.Name -eq "multica-desktop.exe")) -or
  (($_.CommandLine -like "*$repo*") -and ($_.CommandLine -like "*@tauri-apps*cli*tauri.js* dev*")) -or
  (($_.CommandLine -like "*$repo*") -and ($_.CommandLine -like "*vite*--port $port*"))
}
foreach ($process in $processes) {
  $ids += [int]$process.ProcessId
}

$ids = $ids | Sort-Object -Unique
foreach ($id in $ids) {
  Stop-Process -Id $id -Force
}

if ($ids.Count -gt 0) {
  "已清理旧 Multica dev 进程: $($ids -join ', ')"
}
`;

  const result = spawnSync('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-Command', script], {
    cwd: repoRoot,
    encoding: 'utf8',
  });

  if (result.stdout.trim()) {
    console.log(result.stdout.trim());
  }
  if (result.status !== 0 && result.stderr.trim()) {
    console.warn(result.stderr.trim());
  }
}

function startTauriDev() {
  const env = { ...process.env };

  if (process.platform === 'win32' && env.USERPROFILE) {
    const pathKey = Object.keys(env).find((key) => key.toLowerCase() === 'path') ?? 'Path';
    env[pathKey] = `${path.join(env.USERPROFILE, '.cargo', 'bin')};${env[pathKey] ?? ''}`;
  }

  const command = process.platform === 'win32' ? 'cmd.exe' : 'npm';
  const args =
    process.platform === 'win32'
      ? ['/d', '/s', '/c', 'npm --workspace apps/desktop run tauri:dev']
      : ['--workspace', 'apps/desktop', 'run', 'tauri:dev'];

  const child = spawn(command, args, {
    cwd: repoRoot,
    env,
    stdio: 'inherit',
  });

  for (const signal of ['SIGINT', 'SIGTERM']) {
    process.on(signal, () => {
      child.kill(signal);
    });
  }

  child.on('exit', (code, signal) => {
    if (signal) {
      process.exit(1);
      return;
    }
    process.exit(code ?? 0);
  });
}

function toPowerShellString(value) {
  return `'${value.replaceAll("'", "''")}'`;
}
