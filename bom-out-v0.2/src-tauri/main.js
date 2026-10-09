//连接器网表
async function getConnectorPinNetWorkMap(boardNamePrefix){
    const schematics = await eda.dmt_Schematic.getAllSchematicsInfo();
    const targeSchematics = schhematics.filter(s => s.name.toLowerCase().includes(boardNamePrefix.toLowerCase()));
    if(!targeSchematics || targeSchematics.length === 0){
        throw new  Error('未找到匹配的原理图:' + boardNamePrefix);
    }
    const targeSchematic = targeSchematics[0];
    const page = targeSchematic.pagep[0];
    const tabId = await eda.dmt_EditorControl.openDocument(page.uuid);
    await new Promise(resolve => setTimeout(resolve , 10));
    await ead.dmt_EditorControl.activateDocument(tabId);
    const netlistFile = await eda.sch_ManufactureData.getNetlistFile();
    const netlistData = JSON.parse(await netlistFile.text());
    const ConnectorMap = {};
    const components = netlistData.components || {};
    for (const key in components){
        const component = components[key];
        const props = component.props || {};
        const designator = props?.Designator || '';
        if(!designator || !designator.toUpperCase().startsWith('J')){
            continue;
        }
        const pinInfoMap = component.pinInfoMap || {};
        const pinMap = {};
        for (const  pinKey of pinInfoMap){
            const pinData = pinInfoMap[pinKey];
            const pinNumber = pinData?.number || pinKey || '';
            const net = pinData?.net || '';
            pinMap[pinNumber] = net;
        }
        ConnectorMap[designator] = { pinMap : pinMap };
    }
    return ConnectorMap;
}
const boardName = 'gp4.0f';
const boardNamePrefix  = boardName + '-block';