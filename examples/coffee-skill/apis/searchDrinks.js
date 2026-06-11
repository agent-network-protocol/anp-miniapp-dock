module.exports = async function searchDrinks(ctx) {
  const query = ctx.arguments.query || 'latte'
  const remote = globalThis.__coffeeRemote

  if (remote && remote.shouldUseRemote(ctx)) {
    const data = await remote.request(ctx, '/api/drinks', {
      method: 'GET',
      query: { query }
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
