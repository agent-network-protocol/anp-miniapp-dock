const skill = wx.modelContext.createSkill(__dirname)

skill.registerAPI('searchDrinks', require('./apis/searchDrinks'))
skill.registerAPI('confirmOrder', require('./apis/confirmOrder'))
skill.registerAPI('payOrder', require('./apis/payOrder'))

module.exports = skill
