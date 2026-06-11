module.exports = async function searchDrinks(ctx) {
  const query = ctx.arguments.query || 'latte'
  const searchQuery = normalizeDrinkQuery(query)
  const remote = globalThis.__coffeeRemote

  if (remote && remote.shouldUseRemote(ctx)) {
    const data = await remote.request(ctx, '/api/drinks', {
      method: 'GET',
      query: { query: searchQuery }
    })
    return {
      isError: false,
      content: [{ type: 'text', text: `Found drinks for ${query} from localhost coffee service` }],
      structuredContent: {
        drinks: data.drinks || []
      },
      _meta: Object.assign({
        componentState: 'drink-list',
      }, remote.authMeta(ctx))
    }
  }

  return {
    isError: false,
    content: [{ type: 'text', text: `Found drinks for ${query}` }],
    structuredContent: {
      drinks: [
        { id: 'latte', name: 'Latte', price: 18 },
        { id: 'americano', name: 'Americano', price: 15 }
      ]
    },
    _meta: {
      componentState: 'drink-list'
    }
  }
}

function normalizeDrinkQuery(query) {
  const text = String(query || '').trim().toLowerCase()
  if (!text) return 'latte'
  if (text.includes('拿铁') || text.includes('latte')) return 'latte'
  if (text.includes('美式') || text.includes('americano')) return 'americano'
  if (text.includes('摩卡') || text.includes('mocha')) return 'mocha'
  if (text === '咖啡' || text === 'coffee' || text.includes('点咖啡') || text.includes('喝咖啡')) {
    return ''
  }
  return text
}
