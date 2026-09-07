// 端到端冒烟：登录 → 首页状态/日志 → 题库 → 设置。用法：node tests/e2e.mjs [baseUrl] [adminToken]
const { chromium } = await import(process.env.PW_CORE || 'playwright-core')

const BASE = process.argv[2] || 'http://127.0.0.1:18080'
const TOKEN = process.argv[3] || 'adm'
const shots = process.env.E2E_SHOTS || '/tmp/zerror-e2e'

const browser = await chromium.launch({
  executablePath: '/opt/google/chrome/chrome',
  headless: true,
  chromiumSandbox: false,
  args: ['--no-sandbox', '--disable-dev-shm-usage'],
})
const page = await browser.newPage({ viewport: { width: 1280, height: 800 } })
const errors = []
page.on('pageerror', e => errors.push(`pageerror: ${e.message}`))
page.on('console', m => { if (m.type() === 'error') errors.push(`console: ${m.text()}`) })
page.on('requestfailed', r => errors.push(`requestfailed: ${r.url()} ${r.failure()?.errorText}`))

const step = async (name, fn) => {
  try {
    await fn()
    console.log(`[ok] ${name}`)
  } catch (e) {
    console.log(`[FAIL] ${name}: ${e.message}`)
    await page.screenshot({ path: `${shots}/fail-${name.replace(/\W+/g, '_')}.png` }).catch(() => {})
  }
}

await step('打开登录页', async () => {
  await page.goto(BASE, { waitUntil: 'networkidle' })
  // 关闭新手引导气泡，避免遮挡
  await page.evaluate(() => localStorage.setItem('tutorial_stepper_finished', '1'))
  await page.waitForSelector('.login-input', { timeout: 10000 })
})

await step('登录', async () => {
  await page.fill('.login-input', TOKEN)
  await page.click('.login-submit')
  await page.waitForSelector('.login-input', { state: 'detached', timeout: 10000 })
  await page.waitForTimeout(1500)
  await page.screenshot({ path: `${shots}/01-home.png` })
})

await step('首页显示服务状态', async () => {
  // 状态由 /api/status 轮询填充，等待最多 10s
  await page.waitForFunction(() => /运行中|running|3\.0\.0/.test(document.body.textContent || ''), null, { timeout: 10000 })
})

await step('触发一次查询并在日志表中看到记录', async () => {
  const res = await page.request.get(`${BASE}/query?title=${encodeURIComponent('凸轮机构中从动件运动规律取决于（ ）。')}&options=${encodeURIComponent('A. 压力角\nB. 传动角\nC. 极力夹角')}&type=single&token=${TOKEN}`)
  const json = await res.json()
  if (json.code !== 1) throw new Error(`查询失败: ${JSON.stringify(json)}`)
  await page.waitForTimeout(1500)
  const text = await page.textContent('body')
  if (!text.includes('凸轮机构')) throw new Error('日志表未出现该请求')
  await page.screenshot({ path: `${shots}/02-home-log.png` })
})

await step('打开题库页并看到题目', async () => {
  await page.locator('.nav-item[title="题库"]').click()
  await page.waitForTimeout(1500)
  const text = await page.textContent('body')
  if (!text.includes('凸轮机构')) throw new Error('题库页未显示题目')
  await page.screenshot({ path: `${shots}/03-questions.png` })
})

await step('打开设置页（模型设置可见平台）', async () => {
  await page.locator('.nav-item[title="设置"]').click()
  await page.waitForTimeout(1200)
  const text = await page.textContent('body')
  if (!/模型|平台/.test(text)) throw new Error('设置页未渲染')
  await page.screenshot({ path: `${shots}/04-settings.png` })
})

await step('常规设置：新增查询令牌', async () => {
  await page.locator('.category-name:has-text("常规设置")').first().click()
  await page.waitForTimeout(800)
  const before = await page.locator('.token-row').count()
  await page.locator('button:has-text("新增令牌")').first().click()
  await page.waitForTimeout(800)
  const after = await page.locator('.token-row').count()
  if (after !== before + 1) throw new Error(`令牌数量 ${before} -> ${after}`)
  const settings = await (await page.request.get(`${BASE}/api/admin/settings`, { headers: { Authorization: `Bearer ${TOKEN}` } })).json()
  if (!settings.multiUser?.users?.length) throw new Error('服务端未保存令牌')
  await page.screenshot({ path: `${shots}/05-general.png` })
})

console.log(errors.length ? `\n浏览器错误 (${errors.length}):\n` + errors.slice(0, 20).join('\n') : '\n无浏览器错误')
await browser.close()
