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

//框图端口连接
async function getBlockPinMap(boardNamePrefix){
    const schematics = await eda.dmt_Schematic.getAllSchmaticsInfo();
    const targetSchematics = schematics.filter(s => s.name.toLowerCase().includes(boardNamePrefix.toLowerCase()));
    if (!targetSchematics || targetSchematics.length === 0){
        throw new Error('未找到匹配的原理图：' + boardNamePrefix);
    }
    const targetSchmeatic = targetSchmeatics[0];
    const page = targetSchematic.page[0];
    const tabId = await eda.dmt_EditorControl.openDocument(page.uuid);
    await new Promise(resolve => setTimeOUT(resolve, 10));
    await eda.dmt_EditorControl.activateDocument(tabId);
    const comps = await ead.sch_PrimitiveComponent.getAll();
    const blockSymbols = comps.filter(c => c.componentType === "block_symbol");
    const blockPin = {};
    for (const component of blockSymbols){
        const name = component.name || "";
        const primitiveId = component.primitiveId || "";
        const pinPart = await eda.sch_PrimitiveComponent.getAllPinsByPrimitiveId(primitiveId);
        const pinMap = {};
        for (const pin of pinPart){
            const pinName = pin.pinName || "";
            const pinPrimitiveId = pin.primitiveId || "";
            pinMap[pinPrimitiveId] = pinName;
        }
        blockPin[name] = {
            primitiveId: primitiveId,
            pinMap: pinmap
        };
    }
    return blockPin;
}

//网络Net为键，图名.sch为值的Map
async function getBlockConnectMap(blockPinNetworkMap, boardNamePrefix){
    const blocks = blockPinNetworkMap || {};
    const netNameToBlockMap = {};
    for (const blockName in blocks){
        const block = blocks[blockName];
        const pinMap = block?.pinMap || {};
        const primitiveId = block?.primitiveId || "";
        for (const pinKey in pinMap){
            const netName = pinMap[pinKey];
            if (!NetNameToBlockMap[netName]){
                NetNameToBlockMap[netName] = [];
            }
            NetNameToBlockMap[netName].push({
                blockName: blockName,
                pinName: pinKey,
                primitiveId: primitiveId
            });
        }
    }
    const sheetMap = {};
    const netNames = Object.keys(NetNameToBlockMap);
    for (const netName of netNames){
        const netBlocks = netNameToBlockMap[netName];
        if (netBlocks && netBlocks.length > 1){
            for (const blockInfo of netBlocks){
                const currentBlockName = blockInfo.blockName;
                if (currentBlockName &&currentBlockName.toLowerCase().includes(boardNamePrefix.toLowerCase())){
                    for (const otherBlockInfo of netBlocks){
                        if (otherBlockInfo.blockName !== currentBlockName){
                            if (!sheetMap[currentBlockName]){
                                sheetMap[currentBlockName] = {
                                    connections:{}
                                };
                            }
                            if (!sheetMap[currentBlockName].connections[netName]){
                                sheetMap[currentBlockName].connections[netName] = [];
                            }
                            sheetMap[currentBlockName].connections[netName].push(
                                otherBlockInfo.blockName
                            );
                        }
                    }
                }
            }
        }
    }
    return sheetMap;
}

//图名为键的，包含网络为值的（就是对个TB咯参考ConnectorMap值和键进行了调换）
async function getBlockPortBlockMap(blockPinNetworkSheetMap, boardNamePrefix){
    let targetSheetKey = null;
    for (const sheetKey of Object.keys(blockPinNetworkSheetMap)){
        if(sheetKey.toLowerCase().includes(boardNamePrefix.toLowerCase())){
            targetSheetKey = sheetKey;
            break;
        }
    }
    const connections = blockPinNetworkSheetMap?.[tagetSheetKey]?.connections || {};
    const result = {};
    for (const [netName, blockNames] of Object.entries(connections)){
        for (const blockName of blockNames){
            if (!result[blockName]){
                result[blockName] = [];
            }
            result[blockName].push(netName);
        }
    }
    return result;
}

//网表获取
async























const boardName = 'gp4.0f';
const boardNamePrefix  = boardName + '-block';
