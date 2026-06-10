Component({
  data: {
    title: 'Choose a drink',
    empty: true,
    drinks: []
  },
  lifetimes: {
    created() {
      const modelCtx = wx.modelContext.getContext(this)
      modelCtx.on(wx.modelContext.NotificationType.Result, (data) => {
        const drinks = data.result.structuredContent.drinks || []
        this.setData({
          drinks,
          empty: drinks.length === 0
        })
      })
    }
  },
  methods: {
    selectDrink(e) {
      this.setData({ selectedDrinkId: e.currentTarget.dataset.id })
    },
    confirmDrink(e) {
      const drinkId = e.currentTarget.dataset.id
      wx.modelContext.getContext(this).sendFollowUpMessage({
        content: [
          { type: 'text', text: `选择 ${drinkId}` },
          {
            type: 'api/call',
            data: {
              name: 'confirmOrder',
              arguments: { drinkId }
            }
          }
        ]
      })
    },
    onImageLoad() {},
    onImageError() {}
  }
})
