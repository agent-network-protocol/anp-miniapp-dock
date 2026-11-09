const skill = wx.modelContext.createSkill(__dirname)

skill.registerAPI('prepareAddressForm', require('./apis/prepareAddressForm'))

module.exports = skill

