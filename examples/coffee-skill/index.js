const skill = wx.modelContext.createSkill(__dirname)

const DEFAULT_REMOTE_BASE_URL = 'http://127.0.0.1:8008'

function remoteBaseUrl(ctx) {
  const args = ctx && ctx.arguments ? ctx.arguments : {}
  return args.remoteBaseUrl || args.serverUrl || DEFAULT_REMOTE_BASE_URL
}

function shouldUseRemote(ctx) {
  const args = ctx && ctx.arguments ? ctx.arguments : {}
  return Boolean(args.remoteBaseUrl || args.serverUrl)
}

function requireWxLoginOk(loginResult) {
  if (!loginResult || loginResult.errMsg !== 'login:ok') {
    const message = loginResult && loginResult.errMsg ? loginResult.errMsg : 'missing login result'
    throw new Error(`coffee DID login failed: ${message}`)
  }

  const didAuth = loginResult.didAuth || {}
  if (didAuth.status && didAuth.status !== 'ok') {
    throw new Error(`coffee DID login failed: ${didAuth.status}`)
  }

  return {
    code: loginResult.code,
    didAuthStatus: didAuth.status || 'ok',
    userDid: didAuth.userDid,
    merchantDid: didAuth.merchantDid
  }
}

async function login(ctx) {
  const baseUrl = remoteBaseUrl(ctx)
  const loginReceipt = requireWxLoginOk(await wx.login())
  const response = await wx.request({
    url: `${baseUrl}/api/login`,
    method: 'POST',
    data: {
      code: loginReceipt.code,
      sessionId: ctx.sessionId,
      skillId: ctx.skillId,
      userDid: ctx.userDid || loginReceipt.userDid,
      agentDid: ctx.agentDid,
      didAuthStatus: loginReceipt.didAuthStatus
    }
  })

  if (response.statusCode < 200 || response.statusCode >= 300) {
    throw new Error(`coffee login failed: ${response.statusCode}`)
  }

  const loginStatus = response.data && (response.data.loginStatus || response.data.status)
  if (loginStatus && loginStatus !== 'ok') {
    throw new Error(`coffee login status not ok: ${loginStatus}`)
  }

  return {
    baseUrl,
    loginStatus: loginStatus || 'ok',
    didAuthStatus: loginReceipt.didAuthStatus
  }
}

async function request(ctx, path, options = {}) {
  const loginSession = await login(ctx)
  const method = options.method || 'GET'
  const response = await wx.request({
    url: `${loginSession.baseUrl}${path}`,
    method,
    data: options.data || options.query || {}
  })

  if (response.statusCode < 200 || response.statusCode >= 300) {
    throw new Error(`coffee HTTP ${method} ${path} failed: ${response.statusCode}`)
  }

  return response.data
}

function authMeta(ctx) {
  return {
    remoteBaseUrl: remoteBaseUrl(ctx),
    remoteLogin: 'wx.login+did-challenge',
    compatLoginEndpoint: '/api/login',
    authBoundary: 'container-managed',
    tokenVisibleToSkill: false,
    requestAuthMode: 'host-managed-bearer'
  }
}

globalThis.__coffeeRemote = Object.freeze({
  login,
  request,
  authMeta,
  remoteBaseUrl,
  shouldUseRemote
})

skill.registerAPI('searchDrinks', require('./apis/searchDrinks'))
skill.registerAPI('confirmOrder', require('./apis/confirmOrder'))
skill.registerAPI('payOrder', require('./apis/payOrder'))

module.exports = skill
