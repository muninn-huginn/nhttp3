/**
 * Load nhttp3-node native addon in Next.js.
 *
 * Uses eval('require') to bypass webpack bundling — this is the
 * standard pattern for native addons in Next.js server code.
 * See: https://github.com/vercel/next.js/issues/36514
 */

let _mod: any = null;

export function getNhttp3() {
  if (_mod) return _mod;
  try {
    // eval('require') prevents webpack from trying to bundle the native addon
    const nodeRequire = eval('require');
    const path = nodeRequire('path');
    _mod = nodeRequire(path.resolve(process.cwd(), '../../nhttp3-node/index.js'));
    return _mod;
  } catch (e: any) {
    console.error('nhttp3-node load error:', e.message);
    return null;
  }
}
