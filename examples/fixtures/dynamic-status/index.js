const skill = wx.modelContext.createSkill(__dirname)

skill.registerAPI('refreshDynamicStatus', require('./apis/refreshDynamicStatus'))

module.exports = skill

