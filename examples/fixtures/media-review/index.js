const skill = wx.modelContext.createSkill(__dirname)

skill.registerAPI('reviewMedia', require('./apis/reviewMedia'))

module.exports = skill

