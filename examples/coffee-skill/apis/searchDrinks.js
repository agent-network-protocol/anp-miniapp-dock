module.exports = async function searchDrinks(ctx) {
  const query = ctx.arguments.query || 'latte'
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
